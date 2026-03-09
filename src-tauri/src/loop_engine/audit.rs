use serde::Deserialize;

/// A proposed obligation from the audit — a specific check the solver MUST complete.
#[derive(Debug, Clone, Deserialize)]
pub struct ObligationProposal {
    pub description: String,
    #[serde(default = "default_obligation_type")]
    pub obligation_type: String,
    #[serde(default = "default_priority")]
    pub priority: f64,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

fn default_obligation_type() -> String {
    "constraint_check".into()
}
fn default_priority() -> f64 {
    0.8
}
fn default_confidence() -> f64 {
    0.7
}

/// Result of an exploration audit — parsed from LLM JSON response.
#[derive(Debug, Clone, Deserialize)]
pub struct AuditResult {
    #[serde(default)]
    pub techniques_explored: Vec<String>,
    #[serde(default)]
    pub techniques_missing: Vec<String>,
    #[serde(default)]
    pub exploration_breadth: f64,
    #[serde(default)]
    pub current_trajectory: String,
    #[serde(default)]
    pub recommended_direction: String,
    #[serde(default)]
    pub should_branch: bool,
    #[serde(default)]
    pub confidence_in_current_path: f64,
    /// When should_branch is true, the audit MUST propose obligations —
    /// specific checks that gate the conclusion until resolved.
    #[serde(default)]
    pub obligations: Vec<ObligationProposal>,
}

/// Build the exploration audit prompt from the current proof state.
pub fn build_audit_prompt(
    problem: &str,
    verified: &[(String, u32, String, String, String)],
    domain: &str,
    research_context: &str,
    all_obligations: &[crate::models::dag::Obligation],
    techniques: &[crate::models::dag::TechniqueEntry],
) -> String {
    let mut p = String::from(
        "You are a proof strategy reviewer. Your job is to evaluate the EXPLORATION BREADTH \
of a proof attempt — not whether the steps are correct (they are all verified), but \
whether the solver has looked in enough different places.\n\n\
A common failure mode: the solver finds one technique that produces verified steps and \
tunnel-visions on it, missing entirely different construction families that could yield \
a stronger result.\n\n",
    );

    // Inject enriched context (prior attempts, research, pattern stats)
    if !research_context.is_empty() {
        p.push_str(research_context);
        p.push('\n');
    }

    p.push_str(&format!("PROBLEM: {}\n", problem));
    p.push_str(&format!("DOMAIN: {}\n\n", domain));

    // Evidence summary — what the verified chain has PROVEN numerically.
    // This anchors the auditor to verified facts, not training priors.
    let evidence_facts = super::evidence::extract_evidence(verified);
    let evidence_summary = super::evidence::build_evidence_summary(&evidence_facts);
    if !evidence_summary.is_empty() {
        p.push_str(&evidence_summary);
        p.push('\n');
    }

    p.push_str(&format!("VERIFIED CHAIN ({} steps):\n", verified.len()));
    for (_, n, _, nat, formal) in verified {
        p.push_str(&format!("  Step {}: {} [formal: {}]\n", n, nat, formal));
    }
    p.push('\n');

    // Obligation history — full picture of what's been covered.
    // The auditor uses this to gauge exploration breadth and avoid re-proposing.
    // Closed obligations ARE the coverage map — show all of them.
    let open_obs: Vec<_> = all_obligations
        .iter()
        .filter(|o| o.status == "open" || o.status == "assigned")
        .collect();
    let closed_obs: Vec<_> = all_obligations
        .iter()
        .filter(|o| o.status != "open" && o.status != "assigned")
        .collect();

    if !open_obs.is_empty() || !closed_obs.is_empty() {
        p.push_str(
            "OBLIGATION HISTORY — this is your coverage map. Each closed obligation represents \
scope that has ALREADY been checked. Do NOT re-propose anything already covered.\n\n",
        );
        if !open_obs.is_empty() {
            p.push_str(&format!(
                "  Still open / in progress ({}):\n",
                open_obs.len()
            ));
            for ob in &open_obs {
                let spent = if ob.steps_spent > 0 {
                    format!(" [{}/{} steps spent]", ob.steps_spent, ob.max_steps)
                } else {
                    String::new()
                };
                p.push_str(&format!(
                    "    - [{}] {}{}\n",
                    ob.obligation_type, ob.description, spent
                ));
            }
        }
        if !closed_obs.is_empty() {
            p.push_str(&format!(
                "  Already resolved / covered ({}):\n",
                closed_obs.len()
            ));
            for ob in &closed_obs {
                let reason = ob
                    .closure_type
                    .as_deref()
                    .or(ob.retraction_reason.as_deref())
                    .unwrap_or(&ob.status);
                p.push_str(&format!(
                    "    - [{}] {} → {}\n",
                    ob.obligation_type, ob.description, reason
                ));
            }
        }
        p.push('\n');
        p.push_str(&format!(
            "  TOTAL SCOPE COVERED: {} obligations resolved, {} still open. \
Only propose obligations that cover GENUINELY NEW ground not represented above.\n\n",
            closed_obs.len(),
            open_obs.len()
        ));
    }

    // Technique registry — what approaches have been tried and their success rates
    if !techniques.is_empty() {
        p.push_str("TECHNIQUE REGISTRY (tracked approaches with success rates):\n");
        for t in techniques.iter().take(10) {
            let ratio = if t.success_count + t.failure_count > 0 {
                format!(
                    " [{}/{}]",
                    t.success_count,
                    t.success_count + t.failure_count
                )
            } else {
                String::new()
            };
            p.push_str(&format!(
                "  - {}: {}{}\n",
                t.technique_family, t.description, ratio
            ));
        }
        p.push('\n');
    }

    p.push_str(
"CRITICAL — EVIDENCE-FIRST OBLIGATION POLICY:\n\
Your obligations MUST be consistent with the verified evidence above.\n\
If the verified chain establishes a lower bound (e.g., c >= 2 from a verified construction),\n\
do NOT propose obligations that assume a smaller bound (e.g., \"prove c <= 3/2\").\n\
Verified computations take precedence over your prior beliefs about the answer.\n\
If you believe the answer is X but the verified chain shows evidence for Y > X,\n\
propose obligations that INVESTIGATE Y, not obligations that assume X.\n\n\
Analyze the exploration breadth and respond with ONLY a JSON object (no markdown fences):\n\
{\n\
  \"techniques_explored\": [\"list of broad technique families used so far\"],\n\
  \"techniques_missing\": [\"technique families NOT yet explored that are relevant to this problem class\"],\n\
  \"exploration_breadth\": 0.0-1.0,\n\
  \"current_trajectory\": \"one-sentence description of where the solver is heading\",\n\
  \"recommended_direction\": \"specific unexplored approach to try next — be concrete\",\n\
  \"should_branch\": true if there is a promising unexplored direction that could yield a DIFFERENT answer,\n\
  \"confidence_in_current_path\": 0.0-1.0,\n\
  \"obligations\": [\n\
    {\n\
      \"description\": \"specific thing that MUST be checked before the proof can conclude\",\n\
      \"obligation_type\": \"CONSTRAINT_CHECK|BOUND|IMPOSSIBILITY|CASE_CHECK|CONSTRUCT|CLASSIFY|COUNT|SYNTHESIZE\",\n\
      \"priority\": 0.0-1.0,\n\
      \"confidence\": 0.0-1.0 (how confident are you this obligation is real, not spurious)\n\
    }\n\
  ]\n\
}\n\n\
IMPORTANT: When should_branch is true, you MUST emit at least one obligation.\n\
Obligations are GATES — they block the solver from concluding until resolved.\n\
They are NOT suggestions. They are mandatory checks.\n\n\
Good obligation: \"Verify the formula 2n-2 holds for permutation σ=(2,4,1,3) with n=4\"\n\
Bad obligation: \"Consider other approaches\" (too vague — must be a concrete, checkable claim)\n\n\
When should_branch is false (exploration is adequate), obligations should be empty [].");
    p
}

/// Parse an exploration audit response from LLM text.
pub fn parse_audit(response: &str) -> Option<AuditResult> {
    super::json_parse::extract_json(response)
}
