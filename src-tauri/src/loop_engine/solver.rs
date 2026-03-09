use super::audit::AuditResult;
use super::step::SuspectedAnswer;
use crate::db::signals::SatisfactionSignal;
use crate::models::council::CouncilFinding;
use crate::models::dag::{Obligation, ProofNode, TechniqueEntry};

/// Filter the verified chain to remove tautological and trivially vacuous steps
/// that add noise without advancing the proof.
fn curate_verified_chain(
    verified: &[(String, u32, String, String, String)],
) -> Vec<&(String, u32, String, String, String)> {
    verified
        .iter()
        .filter(|(_, _, _, _, formal)| {
            // Skip exact tautologies: LHS == RHS after trimming
            if let Some((lhs, rhs)) = formal.split_once('=') {
                let l = lhs.trim();
                let r = rhs.trim();
                if l == r {
                    return false;
                }
                // Skip X - X = 0 pattern (trivially vacuous subtraction)
                if r == "0" && l.contains(" - (") && l.ends_with(')') {
                    // e.g., "b - k**k - (b - k**k)"
                    return false;
                }
            }
            true
        })
        .collect()
}

/// Cap and deduplicate obligations for prompt injection.
/// RESOLVE obligations are capped, structural obligations get priority.
fn curate_obligations(obligations: &[Obligation]) -> Vec<&Obligation> {
    const MAX_OBLIGATIONS_IN_PROMPT: usize = 8;
    const MAX_RESOLVE_IN_PROMPT: usize = 3;

    let mut structural: Vec<&Obligation> = Vec::new();
    let mut resolve: Vec<&Obligation> = Vec::new();
    let mut resolve_seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for ob in obligations {
        if ob.obligation_type.eq_ignore_ascii_case("RESOLVE") {
            // Deduplicate RESOLVE by description hash (many are rephrased duplicates)
            let key = ob.description.chars().take(60).collect::<String>();
            if resolve_seen.contains(&key) {
                continue;
            }
            resolve_seen.insert(key);
            resolve.push(ob);
        } else {
            structural.push(ob);
        }
    }

    // Structural obligations first, then capped RESOLVE
    let mut result: Vec<&Obligation> = structural;
    result.extend(resolve.into_iter().take(MAX_RESOLVE_IN_PROMPT));
    result.truncate(MAX_OBLIGATIONS_IN_PROMPT);
    result
}

fn append_suspected_answer_context(
    prompt: &mut String,
    suspected_answer: Option<&SuspectedAnswer>,
    contradiction_guidance: &str,
) {
    let Some(sa) = suspected_answer else {
        return;
    };

    if !sa.disproved {
        prompt.push_str(&format!(
            "SUSPECTED ANSWER (source: {}, confidence: {:.0}%):\n\
             External reconnaissance suggests the answer may be: {}\n\
             Treat this as a hypothesis to VERIFY, not a given fact.\n\
             {}\n\n",
            sa.source,
            sa.confidence * 100.0,
            sa.value,
            contradiction_guidance
        ));
        return;
    }

    prompt.push_str(&format!(
        "DISPROVED HYPOTHESIS: The previously suspected answer '{}' has been disproved.\n\
         Reason: {}\n\
         Do NOT target this value. Follow where the mathematics leads.\n\n",
        sa.value,
        sa.disproval_reason
            .as_deref()
            .unwrap_or("contradicted by verified step")
    ));
}

fn append_research_context(prompt: &mut String, research_context: &str) {
    if research_context.is_empty() {
        return;
    }

    prompt.push_str(research_context);
    prompt.push_str("\n\n");
}

fn append_attempt_constraints(prompt: &mut String, heading: &str, attempt_constraints: &[String]) {
    if attempt_constraints.is_empty() {
        return;
    }

    prompt.push_str(heading);
    for constraint in attempt_constraints {
        prompt.push_str(&format!("  - {}\n", constraint));
    }
    prompt.push('\n');
}

fn append_technique_registry(
    prompt: &mut String,
    heading: &str,
    techniques: &[TechniqueEntry],
    limit: usize,
) {
    if techniques.is_empty() {
        return;
    }

    prompt.push_str(heading);
    for technique in techniques.iter().take(limit) {
        let ratio = if technique.success_count + technique.failure_count > 0 {
            format!(
                " [{}/{}]",
                technique.success_count,
                technique.success_count + technique.failure_count
            )
        } else {
            String::new()
        };
        prompt.push_str(&format!(
            "  - {}: {}{}\n",
            technique.technique_family, technique.description, ratio
        ));
    }
    prompt.push('\n');
}

fn obligation_status_label(status: &str) -> &str {
    match status {
        "open" | "assigned" => "OPEN",
        "closed_proved" => "PROVED",
        "closed_spurious" => "SPURIOUS",
        "closed_retracted" => "RETRACTED",
        _ => "CLOSED",
    }
}

/// Build solver prompt from proof state, with optional audit findings and prior review.
pub(super) fn build_solver_prompt(
    problem: &str,
    verified: &[(String, u32, String, String, String)],
    failures: &[(String, String)],
    patterns: &[(String, String, String)],
    audit: Option<&AuditResult>,
    prior_findings: &[CouncilFinding],
    open_obligations: &[Obligation],
    all_obligations: &[Obligation],
    attempt_constraints: &[String],
    techniques: &[TechniqueEntry],
    research_context: &str,
    stuck_steps: u32,
    suspected_answer: Option<&SuspectedAnswer>,
) -> String {
    let verified_count = verified.len();
    let mut p = String::from(
"You are a mathematical proof assistant operating in a STEP-BY-STEP loop.\n\
You may produce ONE or MULTIPLE proof steps per response.\n\
- For a SINGLE step: respond with one JSON object.\n\
- For MULTIPLE steps: respond with a JSON ARRAY of objects, each a self-contained step.\n\
  Each step is validated independently — if one fails, later steps are still processed.\n\
  Use batch mode when you can derive several verified facts in one chain of reasoning.\n\n\
CRITICAL — FIRST PRINCIPLES ONLY:\n\
- Derive ALL claims from scratch using the problem statement and verified steps.\n\
- Do NOT import answers, conjectures, or results from your training data.\n\
- Do NOT assume you know the answer. If prior steps suggest a specific value, VERIFY it computationally.\n\
- If you recognize this problem from competitions or papers, IGNORE that knowledge.\n\
  Your memory of the answer may be wrong. Only trust what you can PROVE step by step.\n\
- Let the algebra lead you. If computations point somewhere unexpected, follow them.\n\n\
RULES:\n\
- proposal_type must be one of: \"algebraic\", \"tactic\", \"computation\", \"lemma\"");

    // Only advertise "conclusion" as a valid type when there are NO open obligations
    if open_obligations.is_empty() {
        p.push_str(
            ", \"conclusion\"\n\
- For conclusion: use proposal_type \"conclusion\" ONLY after ALL obligations are closed.\n",
        );
    } else {
        p.push_str(&format!("\n\
- \"conclusion\" is UNAVAILABLE — {} open obligation(s) remain. Do NOT use proposal_type \"conclusion\".\n\
  The system will silently discard any conclusion attempt. Work on obligations instead.\n", open_obligations.len()));
    }
    p.push_str("\
- formal uses SymPy syntax. EQUALITY claims use a single = sign.\n\
  CRITICAL: SymPy verifies IDENTITIES — equalities true for ALL values of the variables.\n\
  It CANNOT verify definitions or variable assignments. Do NOT introduce new variable names.\n\
  If you want to substitute, do the substitution inline on both sides.\n\
  Good: \"x**2 - 1 = (x-1)*(x+1)\"  (algebraic identity, true for all x)\n\
  Good: \"Sum(k, (k, 1, n)) = n*(n+1)/2\"  (SymPy Sum is supported)\n\
  Good: \"cos(2*x) = 2*cos(x)**2 - 1\"  (trig identity — uses ORIGINAL variable x, no shorthand)\n\
  Bad:  \"s = a + b\" (DEFINITION — introduces new symbol s; SymPy sees s as independent)\n\
  Bad:  \"u = cos(x)\" (SUBSTITUTION — SymPy treats u and cos(x) as completely unrelated)\n\
        --> CORRECT: write identities DIRECTLY using original variables.\n\
  NEVER use u, v, s, t as abbreviations — always expand fully with original problem variables.\n\
  Bad:  \"let x = 5\" or \"define f(x) = x**2\" (definitions, not identities)\n\
  IMPORTANT: Use SymPy functions, NOT Python builtins:\n\
    - Sums: Sum(expr, (var, start, end))    NOT sum() or range()\n\
    - Floor: floor(x)                       NOT x//y\n\
    - Factorial: factorial(n)               NOT math.factorial(n)\n\
    - Binomial: binomial(n, k)              NOT comb(n, k)\n\
    - Equality: single = sign              NOT ==\n\n\
TYPED CLAIMS — for non-equality claims, add a \"claim\" object alongside formal:\n\n\
  EQUALITY (default — use formal field only, no claim object needed):\n\
    formal: \"x**2 - 1 = (x-1)*(x+1)\"\n\n\
  DIVISIBILITY (a divides b):\n\
    formal: \"a | b**a\"\n\
    claim: {\"type\": \"divisibility\", \"dividend\": \"b**a\", \"divisor\": \"a\"}\n\n\
  INEQUALITY:\n\
    formal: \"f(n) <= n\"\n\
    claim: {\"type\": \"inequality\", \"lhs\": \"f(n)\", \"rhs\": \"n\", \"relation\": \"<=\"}\n\n\
  GCD:\n\
    formal: \"gcd(21*n+4, 14*n+3) = 1\"\n\
    claim: {\"type\": \"gcd\", \"a\": \"21*n+4\", \"b\": \"14*n+3\", \"value\": \"1\"}\n\n\
  CONGRUENCE:\n\
    formal: \"2**10 ≡ 2 (mod 7)\"\n\
    claim: {\"type\": \"congruence\", \"expr\": \"2**10\", \"remainder\": \"2\", \"modulus\": \"7\"}\n\n\
  FOR_ALL:\n\
    formal: \"for all k in 1..n: k | n!\"\n\
    claim: {\"type\": \"for_all\", \"variable\": \"k\", \"domain\": \"1..n\",\n\
            \"predicate\": \"Mod(factorial(n), k) == 0\"}\n\n\
  When claim is present, the typed verifier handles it directly — no equality conversion needed.\n\
  Steps without a valid formal or claim will be REJECTED.\n\
- formal_lean: A SEPARATE Lean 4 expression (optional). Omit if unsure.\n\
- Each algebraic step should prove a DISTINCT fact. Don't restate what's already verified.\n\
- If a step is rejected, change your approach — don't rephrase the same step.\n\
- Respond with ONLY a single JSON object. No markdown fences, no commentary, no extra text.\n\n\
TOOLS AVAILABLE — call these before submitting your step:\n\
\n\
  VERIFICATION (call before submitting):\n\
  • sympy_check(lhs, rhs) — for equality claims:\n\
      Split your formal at the = sign and call this.\n\
      Example: formal=\"x**2-1=(x-1)*(x+1)\" → sympy_check(lhs=\"x**2-1\", rhs=\"(x-1)*(x+1)\")\n\
      - EQUAL → correct, submit it.  NOT EQUAL + diff → fix the formal.\n\
  • claim_check(claim) — for typed claims (divisibility, inequality, gcd, congruence, for_all):\n\
      Pass your claim object to verify before submitting.\n\
      Example: claim_check(claim={\"type\": \"divisibility\", \"dividend\": \"b**a\", \"divisor\": \"a\"})\n\
      - VERIFIED → correct, submit.  NOT VERIFIED → fix the claim.\n\
\n\
  LEAN 4 KERNEL (proof-level verification scratch pad):\n\
  • lean_check(lhs, rhs, variables?) — kernel-verify an equality via Lean 4 + Mathlib.\n\
      Uses ring/omega/norm_num/simp/decide. If Lean says VERIFIED, it's mathematically proven.\n\
      Example: lean_check(lhs=\"x^2 - 1\", rhs=\"(x-1)*(x+1)\", variables=[\"x\"])\n\
  • lean_cmd(cmd, env?) — send any Lean 4 command: #check, def, theorem with sorry, etc.\n\
      Use this to explore types, define helper lemmas, or prototype proofs.\n\
  • lean_tactic(tactic, proof_state) — apply a tactic to close a proof goal.\n\
      First create a theorem with sorry via lean_cmd, then apply tactics step by step.\n\
\n\
  RESEARCH (actively use these to find techniques, theorems, and prior results):\n\
  • tavily_search(query)             – AI web search: finds solutions, techniques, and explanations\n\
  • arxiv_search(query)              – search arXiv for relevant math papers and proofs\n\
  • oeis_lookup(query)               – look up integer sequences by terms or description\n\
  • wolfram_query(query)             – compute or look up symbolic expressions\n\
  • semantic_scholar_search(query)   – search academic papers with citation data\n\
  • loogle_search(query)             – search Lean 4 Mathlib for formal theorems\n\
  • get_paper(source, id)            – fetch full details of a specific paper\n\
  • get_sequence(id)                 – fetch full OEIS sequence details\n\
\n\
  COST EFFICIENCY: Each rejected step wastes budget. Research BEFORE attempting a step to maximize\n\
  your chance of getting it right the first time. One research call that finds the right technique\n\
  saves 3-5 failed attempts. Use research tools early and often — they are cheap compared to retries.\n\
  • Before a new obligation: tavily_search or arxiv_search for known techniques\n\
  • When stuck: semantic_scholar_search for related results, loogle_search for Lean lemmas\n\
  • For computations: wolfram_query to confirm values before formalizing\n\
  • For sequences: oeis_lookup to identify patterns and find known formulas\n\
\n\
  WORKFLOW: reason → RESEARCH (find techniques/theorems) → write formal/claim → verify → fix if needed → submit JSON.\n\
  BUDGET: You have a maximum of 15 tool calls per step. Prioritize 1-2 research calls + 1 verification call.\n\
  After verifying, produce your final JSON immediately — do not loop.\n\
  If the check returns SKIPPED or you cannot fix the error in 2-3 attempts, submit your best version.\n\n");

    p.push_str(&format!("PROBLEM: {}\n\n", problem));

    // Suspected answer — from reconnaissance or DB. This is a HYPOTHESIS, not a fact.
    append_suspected_answer_context(
        &mut p,
        suspected_answer,
        "If your computations contradict this value, follow the math — the hypothesis may be wrong.",
    );

    // Research context — from automated pre-solve literature search
    append_research_context(&mut p, research_context);

    // Attempt constraints — structural rules for this attempt that override solver preferences
    append_attempt_constraints(
        &mut p,
        "ATTEMPT CONSTRAINTS (you MUST follow these rules — they are non-negotiable):\n",
        attempt_constraints,
    );

    // Open obligations — GATES that must be resolved before concluding
    // Curated: structural obligations first, RESOLVE capped and deduped
    let curated_obligations = curate_obligations(open_obligations);
    if !curated_obligations.is_empty() {
        let total_open = open_obligations.len();
        let shown = curated_obligations.len();
        p.push_str(&format!(
            "OPEN OBLIGATIONS ({} shown of {} total — must be resolved before conclusion):\n",
            shown, total_open
        ));
        p.push_str("Your next step MUST address one of these. \"conclusion\" is DISABLED until all are resolved.\n");
        for ob in &curated_obligations {
            let priority_label = if ob.priority >= 0.8 {
                "HIGH"
            } else if ob.priority >= 0.5 {
                "MED"
            } else {
                "LOW"
            };
            p.push_str(&format!(
                "  [{}] [{}] {}\n",
                priority_label, ob.obligation_type, ob.description
            ));
        }
        p.push('\n');
    }

    // Closed obligations — show what has ALREADY been resolved so the solver doesn't redo it
    let closed: Vec<&Obligation> = all_obligations
        .iter()
        .filter(|o| o.status != "open" && o.status != "assigned")
        .collect();
    if !closed.is_empty() {
        p.push_str(&format!(
            "RESOLVED OBLIGATIONS ({} closed — do NOT rework these):\n",
            closed.len()
        ));
        for ob in closed.iter().take(8) {
            p.push_str(&format!(
                "  [{}] [{}] {}\n",
                obligation_status_label(ob.status.as_str()),
                ob.obligation_type,
                ob.description
            ));
        }
        if closed.len() > 8 {
            p.push_str(&format!("  ... and {} more resolved\n", closed.len() - 8));
        }
        p.push_str("These obligations are DONE. Focus only on OPEN obligations above.\n\n");
    }

    // Evidence accumulator — extract what the verified chain has PROVEN
    // and check for conflicts with open obligations.
    // This is the key mechanism for defeating training contamination:
    // the solver sees its own verified evidence summarized and any
    // obligations that contradict that evidence are flagged.
    let evidence_facts = super::evidence::extract_evidence(verified);
    let evidence_summary = super::evidence::build_evidence_summary(&evidence_facts);
    if !evidence_summary.is_empty() {
        p.push_str(&evidence_summary);
        p.push('\n');
    }
    let conflicts = super::evidence::find_obligation_conflicts(&evidence_facts, open_obligations);
    let conflict_warning = super::evidence::build_conflict_warning(&conflicts);
    if !conflict_warning.is_empty() {
        p.push_str(&conflict_warning);
        p.push('\n');
    }

    // Inject prior review findings — tells the solver what was missed in previous attempts
    if !prior_findings.is_empty() {
        p.push_str("PRIOR ANALYSIS (from review of previous attempt — address these gaps):\n");
        for f in prior_findings.iter().take(5) {
            p.push_str(&format!(
                "  [{}] {}: {}\n",
                f.finding_type, f.summary, f.detail
            ));
        }
        p.push('\n');
    }

    // Inject exploration audit findings — mid-proof breadth check
    if let Some(a) = audit {
        p.push_str(&format!(
            "EXPLORATION AUDIT (breadth score: {:.1}/1.0):\n\
             Techniques explored: {}\n\
             Techniques NOT explored: {}\n\
             Recommended direction: {}\n\
             WARNING: Your current path may be locally optimal but globally incomplete.\n\
             Consider the recommended direction before continuing the current approach.\n\n",
            a.exploration_breadth,
            a.techniques_explored.join(", "),
            a.techniques_missing.join(", "),
            a.recommended_direction,
        ));
    }

    // Verified chain — curated to remove tautological noise
    let curated_chain = curate_verified_chain(verified);
    if !curated_chain.is_empty() {
        let skipped = verified_count - curated_chain.len();
        if skipped > 0 {
            p.push_str(&format!(
                "VERIFIED STEPS ({} substantive of {} total, {}/{} needed for conclusion):\n",
                curated_chain.len(),
                verified_count,
                verified_count,
                3
            ));
        } else {
            p.push_str(&format!(
                "VERIFIED STEPS SO FAR ({}/{} needed for conclusion):\n",
                verified_count, 3
            ));
        }
        // Show last 15 curated steps to keep context focused
        let display_chain: Vec<_> = if curated_chain.len() > 15 {
            let skipped_steps = curated_chain.len() - 15;
            p.push_str(&format!(
                "  ... ({} earlier steps omitted)\n",
                skipped_steps
            ));
            curated_chain[curated_chain.len() - 15..].to_vec()
        } else {
            curated_chain
        };
        for (_, n, _, nat, formal) in display_chain {
            p.push_str(&format!("  Step {}: {} [formal: {}]\n", n, nat, formal));
        }
        p.push('\n');
    } else if verified_count > 0 {
        // All steps were tautological — report the count but not the content
        p.push_str(&format!(
            "{} verified steps exist but all are trivial identities. Prove a SUBSTANTIVE algebraic equality.\n\n",
            verified_count
        ));
    } else {
        p.push_str("No verified steps yet. Start by proving a concrete algebraic equality.\n\n");
    }
    // Recent failures — cap to prevent prompt bloat
    if !failures.is_empty() {
        p.push_str("REJECTED STEPS (do NOT repeat these):\n");
        for (prop, reason) in failures.iter().take(8) {
            p.push_str(&format!("  \"{}\" -- reason: {}\n", prop, reason));
        }
        if failures.len() > 8 {
            p.push_str(&format!(
                "  ... and {} more rejected steps\n",
                failures.len() - 8
            ));
        }
        p.push('\n');
    }
    if !patterns.is_empty() {
        p.push_str("KNOWN PATTERNS:\n");
        for (name, trigger, strategy) in patterns {
            p.push_str(&format!("  '{}': {} -> {}\n", name, trigger, strategy));
        }
        p.push('\n');
    }
    append_technique_registry(
        &mut p,
        "TECHNIQUE REGISTRY (known approaches for this problem class):\n",
        techniques,
        10,
    );
    // Meta-strategy hint when solver is stuck on same obligation ≥5 steps.
    // Injects named classical strategies to break out of local exploration loops.
    if stuck_steps >= 5 {
        p.push_str(&format!(
            "STRATEGIC HINT (you have been working on the same goal for {} steps without progress):\n\
             Consider one of these classical techniques if you haven't tried it yet:\n\
             - INDUCTION: Prove a base case, then an inductive step (n → n+1)\n\
             - CONTRADICTION: Assume the negation of the goal, derive an impossibility\n\
             - PIGEONHOLE: If N+1 objects fit into N containers, some container holds ≥2\n\
             - EXTREMAL: Consider the minimal or maximal element with property P\n\
             - INVARIANT: Find a quantity preserved or strictly monotone under the process\n\
             - CAUCHY-SCHWARZ / AM-GM: Standard inequality machinery for bound problems\n\
             - DIAGONALIZATION: Construct a counterexample by diagonalizing over all cases\n\
             Try a FUNDAMENTALLY DIFFERENT approach from your recent steps.\n\n",
            stuck_steps
        ));
    }

    p.push_str(
"THINKING INSTRUCTIONS: Use your thinking/reasoning phase to work through the mathematics fully.\n\
Only after completing your reasoning, output the JSON. Do NOT write the formal field until you have\n\
verified the algebra in your thinking. Reasoning comes first — formal output comes last.\n\n\
OUTPUT: Exactly one JSON object — the SINGLE next step. You will be called again for the step after that.\n\
{\"proposal_type\": \"algebraic\", \"reasoning\": \"<why this step — work through the algebra here>\", \"natural\": \"<human readable step>\", \"formal\": \"<SymPy equality>\", \"formal_lean\": \"<Lean 4 equality, optional>\"}");
    p
}

/// Build a solver prompt focused on a SPECIFIC obligation.
///
/// Unlike `build_solver_prompt` (which lists all obligations as advisory hints),
/// this gives the solver a single mission: resolve this obligation.
pub(super) fn build_obligation_solver_prompt(
    problem: &str,
    obligation: &Obligation,
    blacklisted_approaches: &[(String, String)],
    verified: &[(String, u32, String, String, String)],
    failures: &[(String, String)],
    attempt_constraints: &[String],
    techniques: &[TechniqueEntry],
    research_context: &str,
    obligation_history: &[ProofNode],
    satisfaction_signals: &[SatisfactionSignal],
    all_obligations: &[Obligation],
    stuck_steps: u32,
    obligation_scout_context: &str,
    suspected_answer: Option<&SuspectedAnswer>,
) -> String {
    let verified_count = verified.len();
    let mut p = String::from(
"You are a mathematical proof assistant operating in a STEP-BY-STEP loop.\n\
You may produce ONE or MULTIPLE proof steps per response.\n\
- For a SINGLE step: respond with one JSON object.\n\
- For MULTIPLE steps: respond with a JSON ARRAY of objects, each a self-contained step.\n\
  Each step is validated independently — if one fails, later steps are still processed.\n\
  Use batch mode when you can derive several verified facts in one chain of reasoning.\n\n\
CRITICAL — FIRST PRINCIPLES ONLY:\n\
- Derive ALL claims from scratch using the problem statement and verified steps.\n\
- Do NOT import answers, conjectures, or results from your training data.\n\
- Do NOT assume you know the answer. If prior steps suggest a specific value, VERIFY it computationally.\n\
- If you recognize this problem from competitions or papers, IGNORE that knowledge.\n\
  Your memory of the answer may be wrong. Only trust what you can PROVE step by step.\n\
- Let the algebra lead you. If computations point somewhere unexpected, follow them.\n\n\
RULES:\n\
- proposal_type must be one of: \"algebraic\", \"tactic\", \"computation\", \"lemma\"\n\
- Do NOT use proposal_type \"conclusion\" — you are working on a sub-obligation, not the final proof.\n\
  A review panel will close this obligation when your cumulative work is sufficient.\n\
- formal uses SymPy syntax. EQUALITY claims use a single = sign.\n\
  CRITICAL: SymPy verifies IDENTITIES — equalities true for ALL values of the variables.\n\
  Do NOT introduce new variable names. Expand fully using original variables.\n\
  Good: \"x**2 - 1 = (x-1)*(x+1)\"  Bad: \"s = a + b\" (definition)\n\
  Use SymPy functions (Sum, floor, factorial, binomial), NOT Python builtins.\n\n\
TYPED CLAIMS — for non-equality claims, add a \"claim\" object alongside formal:\n\
  DIVISIBILITY: claim: {\"type\": \"divisibility\", \"dividend\": \"b**a\", \"divisor\": \"a\"}\n\
  INEQUALITY:   claim: {\"type\": \"inequality\", \"lhs\": \"f(n)\", \"rhs\": \"n\", \"relation\": \"<=\"}\n\
  GCD:          claim: {\"type\": \"gcd\", \"a\": \"21*n+4\", \"b\": \"14*n+3\", \"value\": \"1\"}\n\
  CONGRUENCE:   claim: {\"type\": \"congruence\", \"expr\": \"2**10\", \"remainder\": \"2\", \"modulus\": \"7\"}\n\
  FOR_ALL:      claim: {\"type\": \"for_all\", \"variable\": \"k\", \"domain\": \"1..n\", \"predicate\": \"Mod(factorial(n), k) == 0\"}\n\
  When claim is present, the typed verifier handles it — no equality conversion needed.\n\
- formal_lean: optional Lean 4 expression. Omit if unsure.\n\
- Respond with ONLY a single JSON object. No markdown fences, no commentary.\n\n\
TOOLS AVAILABLE — call these before submitting your step:\n\
\n\
  VERIFICATION (call before submitting):\n\
  • sympy_check(lhs, rhs) — for equality claims: split at = sign, verify.\n\
  • claim_check(claim) — for typed claims (divisibility, inequality, gcd, congruence, for_all).\n\
\n\
  LEAN 4 KERNEL (proof-level verification scratch pad):\n\
  • lean_check(lhs, rhs, variables?) — kernel-verify equalities via Lean 4 + Mathlib.\n\
  • lean_cmd(cmd, env?) — send Lean 4 commands: #check, def, theorem with sorry.\n\
  • lean_tactic(tactic, proof_state) — apply tactics to close proof goals.\n\
\n\
  RESEARCH (actively use these to find techniques, theorems, and prior results):\n\
  • tavily_search(query)             — AI web search: finds solutions, techniques, and explanations\n\
  • arxiv_search(query)              — search arXiv for relevant math papers and proofs\n\
  • oeis_lookup(query)               — look up integer sequences by terms or description\n\
  • wolfram_query(query)             — compute or look up symbolic expressions\n\
  • semantic_scholar_search(query)   — search academic papers with citation data\n\
  • loogle_search(query)             — search Lean 4 Mathlib for formal theorems\n\
  • get_paper(source, id)            — fetch full details of a specific paper\n\
  • get_sequence(id)                 — fetch full OEIS sequence details\n\
\n\
  COST EFFICIENCY: Each rejected step wastes budget. Research BEFORE attempting a step to maximize\n\
  your chance of getting it right the first time. One research call that finds the right technique\n\
  saves 3-5 failed attempts. Use research tools early and often — they are cheap compared to retries.\n\
  • Before a new obligation: tavily_search or arxiv_search for known techniques\n\
  • When stuck: semantic_scholar_search for related results, loogle_search for Lean lemmas\n\
  • For computations: wolfram_query to confirm values before formalizing\n\
  • For sequences: oeis_lookup to identify patterns and find known formulas\n\
\n\
  WORKFLOW: reason → RESEARCH (find techniques/theorems) → write formal/claim → verify → fix if needed → submit JSON.\n\
  BUDGET: You have a maximum of 15 tool calls per step. Prioritize 1-2 research calls + 1 verification call.\n\
  After verifying, produce your final JSON immediately — do not loop.\n\n");

    p.push_str(&format!("PROBLEM: {}\n\n", problem));

    // Suspected answer — from reconnaissance or DB
    append_suspected_answer_context(
        &mut p,
        suspected_answer,
        "If your computations contradict this value, follow the math.",
    );

    // Research context — from automated pre-solve literature search
    append_research_context(&mut p, research_context);

    // Obligation-specific scout results — from mid-solve targeted research
    if !obligation_scout_context.is_empty() {
        p.push_str("TARGETED RESEARCH FOR THIS OBLIGATION:\n");
        p.push_str(obligation_scout_context);
        p.push_str("\n\n");
    }

    // === THE MISSION ===
    let priority_label = if obligation.priority >= 0.95 {
        "CRITICAL"
    } else if obligation.priority >= 0.8 {
        "HIGH"
    } else if obligation.priority >= 0.5 {
        "MEDIUM"
    } else {
        "LOW"
    };
    let type_hint = match obligation.obligation_type.as_str() {
        "COUNT" | "count" => "Establish a cardinality or quantity via an algebraic identity.",
        "CLASSIFY" | "classify" => "Characterize a family of objects — cover all cases.",
        "CONSTRUCT" | "construct" | "construction" => {
            "Build an explicit example or configuration and verify it."
        }
        "BOUND" | "bound" => "Prove an inequality or extremal result.",
        "IMPOSSIBILITY" | "impossibility" => {
            "Show something cannot exist — derive a contradiction."
        }
        "CASE_CHECK" | "case_check" | "constraint_check" => {
            "Verify a specific small case or constraint."
        }
        "RESOLVE" | "resolve" => "Reconcile a contradiction between two established steps.",
        "SYNTHESIZE" | "synthesize" => "Combine results into a coherent final statement.",
        _ => "Advance the proof toward this subgoal.",
    };

    p.push_str(&format!(
        "YOUR MISSION [{}]: {}\n\
         Type: {} — {}\n\
         Progress: {} of {} steps used\n",
        priority_label,
        obligation.description,
        obligation.obligation_type,
        type_hint,
        obligation.steps_spent,
        obligation.max_steps,
    ));
    if let Some(ref criteria) = obligation.satisfaction_criteria {
        p.push_str(&format!("  Done when: {}\n", criteria));
    }
    p.push_str(
        "\n\
         This obligation is a MINI-PROOF. You do NOT need to close it in one step.\n\
         Each verified step contributes to a cumulative chain. A panel of reviewers\n\
         evaluates the accumulated work each round — when the chain is sufficient,\n\
         they will close the obligation. Focus on producing ONE sound, verifiable step\n\
         that advances the proof. Build on prior verified steps.\n\n",
    );

    // Sibling obligations — show what's open vs closed so the solver has the full picture
    let siblings: Vec<&Obligation> = all_obligations
        .iter()
        .filter(|o| o.id != obligation.id)
        .collect();
    if !siblings.is_empty() {
        let open_count = siblings
            .iter()
            .filter(|o| o.status == "open" || o.status == "assigned")
            .count();
        let closed_count = siblings.len() - open_count;
        p.push_str(&format!(
            "OTHER OBLIGATIONS ({} open, {} closed):\n",
            open_count, closed_count
        ));
        for ob in &siblings {
            p.push_str(&format!(
                "  [{}] [{}] {}\n",
                obligation_status_label(ob.status.as_str()),
                ob.obligation_type,
                ob.description
            ));
        }
        p.push_str(
            "Do NOT rework PROVED/CLOSED obligations. Focus only on YOUR MISSION above.\n\n",
        );
    }

    // Attempt constraints
    append_attempt_constraints(
        &mut p,
        "ATTEMPT CONSTRAINTS (non-negotiable):\n",
        attempt_constraints,
    );

    // Blacklisted approaches — pivot forcing
    if !blacklisted_approaches.is_empty() {
        p.push_str("DO NOT USE these approaches (previously failed 3+ times):\n");
        for (approach, reason) in blacklisted_approaches {
            p.push_str(&format!("  - {}: {}\n", approach, reason));
        }
        p.push_str("You MUST try a DIFFERENT technique.\n\n");
    }

    // Obligation step history — what the solver already tried for THIS obligation
    if !obligation_history.is_empty() {
        let verified_count = obligation_history
            .iter()
            .filter(|n| n.status == "verified")
            .count();
        let rejected_count = obligation_history
            .iter()
            .filter(|n| n.status == "rejected")
            .count();
        p.push_str(&format!(
            "PRIOR ATTEMPTS ON THIS OBLIGATION ({} steps: {} verified, {} rejected):\n",
            obligation_history.len(),
            verified_count,
            rejected_count,
        ));
        // Show last 8 entries
        let display: Vec<_> = if obligation_history.len() > 8 {
            p.push_str(&format!(
                "  ... ({} earlier steps omitted)\n",
                obligation_history.len() - 8
            ));
            obligation_history[obligation_history.len() - 8..].to_vec()
        } else {
            obligation_history.to_vec()
        };
        for node in &display {
            let status_label = if node.status == "verified" {
                "VERIFIED"
            } else {
                "REJECTED"
            };
            let formal_str = node.formal_content.as_deref().unwrap_or("none");
            let content_preview: String = node.content.chars().take(100).collect();
            // Extract rejection reason from validator_result JSON if available
            let rejection_info = if node.status == "rejected" {
                node.validator_result
                    .as_deref()
                    .and_then(|vr| serde_json::from_str::<serde_json::Value>(vr).ok())
                    .and_then(|v| {
                        // Try rejection_reason or sympy.message
                        v.get("rejection_reason")
                            .and_then(|r| r.as_str().map(|s| s.to_string()))
                            .or_else(|| {
                                v.get("sympy")
                                    .and_then(|s| s.get("message"))
                                    .and_then(|m| m.as_str().map(|s| s.to_string()))
                            })
                    })
                    .map(|r| format!(" — {}", r))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            p.push_str(&format!(
                "  Step {} [{}]: \"{}\" [formal: {}]{}\n",
                node.sequence_number, status_label, content_preview, formal_str, rejection_info,
            ));
        }
        if rejected_count > 0 {
            p.push_str("Do NOT repeat rejected approaches. Build on verified steps toward closing this obligation.\n");
        }
        p.push('\n');

        // Explicit rejected formalization blacklist — prevents resubmission of equivalent expressions
        let rejected_formals: Vec<&str> = obligation_history
            .iter()
            .filter(|n| n.status == "rejected")
            .filter_map(|n| n.formal_content.as_deref())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        if rejected_formals.len() >= 2 {
            p.push_str("\u{26d4} REJECTED FORMALIZATIONS \u{2014} DO NOT RESUBMIT:\n");
            for formal in &rejected_formals {
                p.push_str(&format!("  \u{2717} {}\n", formal));
            }
            p.push_str("These exact expressions were already rejected by the CAS. ");
            p.push_str("You MUST use a structurally different formalization.\n\n");
        }
    }

    // Satisfaction feedback — show only the LATEST round's votes (most recent step_id)
    if !satisfaction_signals.is_empty() {
        // Find the latest step_id and filter to only that round
        let latest_step_id = satisfaction_signals
            .iter()
            .rev()
            .find_map(|s| s.step_id.as_ref());
        let recent: Vec<_> = if let Some(sid) = latest_step_id {
            satisfaction_signals
                .iter()
                .filter(|s| s.step_id.as_ref() == Some(sid))
                .cloned()
                .collect()
        } else {
            // No step_id — fall back to last 3
            satisfaction_signals
                .iter()
                .rev()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect()
        };
        if !recent.is_empty() {
            let yes_count = recent.iter().filter(|s| s.satisfies).count();
            let no_count = recent.len() - yes_count;
            p.push_str(&format!(
                "LAST ROUND FEEDBACK ({} yes, {} no — 2/3 needed to close):\n",
                yes_count, no_count,
            ));
            for sig in &recent {
                let verdict = if sig.satisfies { "YES" } else { "NO" };
                let note_str = sig.note.as_deref().unwrap_or("");
                p.push_str(&format!(
                    "  [{}] {} {}\n",
                    sig.source.to_uppercase(),
                    verdict,
                    note_str
                ));
            }
            if no_count > 0 {
                p.push_str("Address the NO votes above — they describe what's still missing.\n");
            }
            p.push('\n');
        }
    }

    // Evidence accumulator — what the verified chain has PROVEN.
    // Critical for obligation prompts: if the mission says "prove c ≤ 3/2"
    // but the evidence shows c ≥ 2, the solver needs to see this conflict.
    {
        let evidence_facts = super::evidence::extract_evidence(verified);
        let evidence_summary = super::evidence::build_evidence_summary(&evidence_facts);
        if !evidence_summary.is_empty() {
            p.push_str(&evidence_summary);
            p.push('\n');
        }
        // Check specifically if THIS obligation conflicts with evidence
        let single_ob = std::slice::from_ref(obligation);
        let conflicts = super::evidence::find_obligation_conflicts(&evidence_facts, single_ob);
        if !conflicts.is_empty() {
            let conflict_warning = super::evidence::build_conflict_warning(&conflicts);
            p.push_str(&conflict_warning);
            p.push('\n');
        }
    }

    // Verified chain — curated to remove tautological noise
    let curated_chain = curate_verified_chain(verified);
    if !curated_chain.is_empty() {
        // Show last 10 curated steps for obligation prompt (keep it focused)
        let display_chain: Vec<_> = if curated_chain.len() > 10 {
            p.push_str(&format!(
                "VERIFIED STEPS ({} substantive, showing last 10):\n",
                curated_chain.len()
            ));
            curated_chain[curated_chain.len() - 10..].to_vec()
        } else {
            p.push_str(&format!(
                "VERIFIED STEPS SO FAR ({}/{} needed for conclusion):\n",
                verified_count, 3
            ));
            curated_chain
        };
        for (_, n, _, nat, formal) in display_chain {
            p.push_str(&format!("  Step {}: {} [formal: {}]\n", n, nat, formal));
        }
        p.push('\n');
    } else {
        p.push_str("No verified steps yet. Start by proving a concrete algebraic equality.\n\n");
    }

    // Recent failures — cap at 5 for obligation prompt
    if !failures.is_empty() {
        p.push_str("REJECTED STEPS (do NOT repeat):\n");
        for (prop, reason) in failures.iter().take(5) {
            p.push_str(&format!("  \"{}\" -- {}\n", prop, reason));
        }
        p.push('\n');
    }

    // Technique registry
    append_technique_registry(&mut p, "TECHNIQUE REGISTRY:\n", techniques, 8);

    // Meta-strategy hint when stuck on this specific obligation
    if stuck_steps >= 5 {
        p.push_str(&format!(
            "STRATEGIC HINT (this obligation has consumed {} steps without closure):\n\
             Consider a fundamentally different technique:\n\
             - INDUCTION: Prove base case, then inductive step (n → n+1)\n\
             - CONTRADICTION: Assume negation, derive impossibility\n\
             - PIGEONHOLE: N+1 objects in N containers → some container holds ≥2\n\
             - EXTREMAL: Consider minimal/maximal element with property P\n\
             - INVARIANT: Quantity preserved or strictly monotone under the process\n\
             - CAUCHY-SCHWARZ / AM-GM: Standard inequality machinery\n\
             - COMPACTNESS: Reduce infinite cases to a finite check\n\
             Your recent steps are not converging — change strategy entirely.\n\n",
            stuck_steps
        ));
    }

    p.push_str(
"THINKING INSTRUCTIONS: Use your thinking/reasoning phase to work through the mathematics fully.\n\
Only after completing your reasoning, output the JSON. Do NOT write the formal field until you have\n\
verified the algebra in your thinking. Reasoning comes first — formal output comes last.\n\n\
OUTPUT: Exactly one JSON object — one proof step (NOT a conclusion).\n\
{\"proposal_type\": \"algebraic\", \"reasoning\": \"<work through the algebra fully here before committing to formal>\",\n \
\"natural\": \"<step description>\", \"formal\": \"<SymPy equality>\", \"formal_lean\": \"<Lean 4, optional>\",\n \
\"targets_obligation\": \"<obligation type, e.g. CASE_CHECK, BOUND, CONSTRUCT>\",\n \
\"closes_obligation\": true/false,\n \
\"closure_reason\": \"<what this step establishes, and what remains to be done>\"}\n\n\
NOTE: closes_obligation is your SELF-ASSESSMENT — a review panel makes the final decision.\n\
Set true only if you believe the cumulative verified chain (not just this step) fully satisfies the mission.\n\
Set false and describe what remains — the panel uses this to evaluate progress.\n\
Do NOT set proposal_type to \"conclusion\" — that is reserved for the final proof, not obligation work.");
    p
}

/// Build a correction prompt when a solver's formal expression fails SymPy pre-check.
///
/// The model receives the exact algebraic diff, which tells it precisely what
/// is wrong (e.g., "your expansion is off by a constant term"). The model then
/// returns a corrected JSON with only the `formal` field changed.
pub fn build_sympy_correction_prompt(
    original_natural: &str,
    original_formal: &str,
    diff: &str,
) -> String {
    format!(
"SymPy (a computer algebra system) checked your formal expression and found it incorrect.\n\n\
You wrote: {original_formal}\n\
SymPy computed LHS - RHS = {diff}\n\n\
This means your algebraic identity does NOT hold for all values of the variables.\n\
Common causes:\n\
  - Off-by-one in an expansion (e.g., forgot a term)\n\
  - Wrong sign in a trig identity (e.g., cos(2x) = 2cos²x - 1, not 2cos²x + 1)\n\
  - Incorrect double-angle or product formula\n\
  - Missing a factor or coefficient\n\n\
Your step's natural language description: {original_natural}\n\n\
Please identify the error in your formal expression and return a corrected version.\n\
Return EXACTLY ONE JSON object with only the corrected formal (all other fields unchanged):\n\
{{\"formal\": \"<corrected SymPy equality>\"}}",
        original_formal = original_formal,
        diff = diff,
        original_natural = original_natural,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_obligation(
        id: &str,
        description: &str,
        obligation_type: &str,
        priority: f64,
        status: &str,
    ) -> Obligation {
        Obligation {
            id: id.to_string(),
            attempt_id: "attempt-1".to_string(),
            branch_id: 0,
            parent_node_id: "node-1".to_string(),
            description: description.to_string(),
            obligation_type: obligation_type.to_string(),
            priority,
            confidence: 0.8,
            source_layer: Some(1),
            status: status.to_string(),
            assigned_model: None,
            closure_node_id: None,
            closure_type: None,
            escalation_level: 0,
            steps_spent: 2,
            max_steps: 8,
            search_space: None,
            superseded_by: None,
            retraction_reason: None,
            depends_on: None,
            decomposition_id: None,
            satisfaction_criteria: None,
            signature_json: None,
            embedding_json: None,
            scout_status: None,
            last_scout_session_id: None,
            last_scout_confidence: None,
            resolved_externally: false,
            resolved_by_corpus_id: None,
            external_reference: None,
            scout_last_checked_at: None,
            assigned_models_json: None,
            active_solver_round_id: None,
            created_at: "2026-03-06T00:00:00Z".to_string(),
            closed_at: None,
        }
    }

    #[test]
    fn solver_prompt_disables_conclusion_when_open_obligations_exist() {
        let open = vec![make_obligation(
            "ob-1",
            "Prove the final bound on c",
            "BOUND",
            0.9,
            "open",
        )];

        let prompt = build_solver_prompt(
            "Determine the optimal constant c.",
            &[],
            &[],
            &[],
            None,
            &[],
            &open,
            &open,
            &[],
            &[],
            "",
            0,
            None,
        );

        assert!(prompt.contains("\"conclusion\" is UNAVAILABLE"));
        assert!(prompt.contains("OPEN OBLIGATIONS"));
        assert!(prompt.contains("Prove the final bound on c"));
    }

    #[test]
    fn solver_prompt_enables_conclusion_when_all_obligations_are_closed() {
        let all = vec![make_obligation(
            "ob-1",
            "Check the extremal construction",
            "CONSTRUCT",
            0.7,
            "closed_proved",
        )];

        let prompt = build_solver_prompt(
            "Determine the optimal constant c.",
            &[],
            &[],
            &[],
            None,
            &[],
            &[],
            &all,
            &[],
            &[],
            "",
            0,
            None,
        );

        assert!(prompt.contains("\"conclusion\""));
        assert!(prompt.contains("use proposal_type \"conclusion\" ONLY"));
        assert!(prompt.contains("RESOLVED OBLIGATIONS"));
        assert!(prompt.contains("[PROVED] [CONSTRUCT] Check the extremal construction"));
    }

    #[test]
    fn solver_prompt_caps_resolve_obligations_after_structural_ones() {
        let open = vec![
            make_obligation(
                "struct-1",
                "Establish the main inequality",
                "BOUND",
                0.9,
                "open",
            ),
            make_obligation(
                "struct-2",
                "Construct a witness family",
                "CONSTRUCT",
                0.8,
                "open",
            ),
            make_obligation("res-1", "Resolve branch conflict 1", "RESOLVE", 0.6, "open"),
            make_obligation("res-2", "Resolve branch conflict 2", "RESOLVE", 0.6, "open"),
            make_obligation("res-3", "Resolve branch conflict 3", "RESOLVE", 0.6, "open"),
            make_obligation("res-4", "Resolve branch conflict 4", "RESOLVE", 0.6, "open"),
        ];

        let prompt = build_solver_prompt(
            "Determine the optimal constant c.",
            &[],
            &[],
            &[],
            None,
            &[],
            &open,
            &open,
            &[],
            &[],
            "",
            0,
            None,
        );

        assert!(prompt.contains("OPEN OBLIGATIONS (5 shown of 6 total"));
        assert!(prompt.contains("Establish the main inequality"));
        assert!(prompt.contains("Construct a witness family"));
        assert!(prompt.contains("Resolve branch conflict 3"));
        assert!(!prompt.contains("Resolve branch conflict 4"));
    }

    #[test]
    fn obligation_prompt_shows_mission_siblings_and_blacklist() {
        let mission = make_obligation(
            "mission",
            "Verify the small exceptional cases",
            "CASE_CHECK",
            0.85,
            "open",
        );
        let sibling_open = make_obligation(
            "sib-open",
            "Establish the asymptotic lower bound",
            "BOUND",
            0.6,
            "assigned",
        );
        let sibling_closed = make_obligation(
            "sib-closed",
            "Construct the extremal family",
            "CONSTRUCT",
            0.7,
            "closed_proved",
        );
        let all = vec![
            mission.clone(),
            sibling_open.clone(),
            sibling_closed.clone(),
        ];

        let prompt = build_obligation_solver_prompt(
            "Determine the optimal constant c.",
            &mission,
            &[(
                "Repeat the same parity split".to_string(),
                "Rejected three times without progress".to_string(),
            )],
            &[],
            &[],
            &["Stay concrete and avoid conclusion claims.".to_string()],
            &[],
            "",
            &[],
            &[],
            &all,
            0,
            "",
            None,
        );

        assert!(prompt.contains("Do NOT use proposal_type \"conclusion\""));
        assert!(prompt.contains("YOUR MISSION [HIGH]: Verify the small exceptional cases"));
        assert!(prompt.contains("OTHER OBLIGATIONS (1 open, 1 closed):"));
        assert!(prompt.contains("[OPEN] [BOUND] Establish the asymptotic lower bound"));
        assert!(prompt.contains("[PROVED] [CONSTRUCT] Construct the extremal family"));
        assert!(prompt.contains("DO NOT USE these approaches"));
        assert!(prompt.contains("Repeat the same parity split"));
        assert!(prompt.contains("ATTEMPT CONSTRAINTS (non-negotiable):"));
    }
}
