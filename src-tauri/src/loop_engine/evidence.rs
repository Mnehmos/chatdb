//! Evidence Accumulator — extracts verified numeric bounds from the proof chain
//! and detects when they contradict open obligations.
//!
//! The core problem this solves: LLM training contamination creates obligations
//! based on memorized (wrong) answers. The solver then verifies computations that
//! DISPROVE those obligations, but the obligation gate stays closed because nothing
//! reads the evidence back. This module closes that loop.
//!
//! It works mechanically — no LLM calls. It scans verified step text for numeric
//! facts (f(n) = k, ratio = r, bound = b) and compares them against obligation
//! descriptions containing numeric claims (c ≤ 3/2, f(n) ≤ cn, etc.).

use crate::models::dag::Obligation;
use regex::Regex;
use std::sync::LazyLock;

/// A numeric fact extracted from a verified step.
#[derive(Debug, Clone)]
pub struct EvidenceFact {
    /// Step number where this was verified.
    pub step_number: u32,
    /// What kind of fact: "function_value", "ratio", "bound", "divisibility"
    pub fact_type: String,
    /// Human-readable summary, e.g. "f(2) = 4 (ratio 2.0)"
    pub summary: String,
    /// If this establishes a lower bound on the answer, what is it?
    pub lower_bound: Option<f64>,
    /// Raw variable name if applicable (e.g. "f(2)")
    pub variable: Option<String>,
    /// Raw numeric value if applicable
    pub value: Option<f64>,
}

/// A detected conflict between verified evidence and an open obligation.
#[derive(Debug, Clone)]
pub struct ObligationConflict {
    /// The obligation that's contradicted.
    pub obligation_id: String,
    /// The obligation description.
    pub obligation_desc: String,
    /// The upper bound claimed by the obligation (e.g. 1.5 for "c ≤ 3/2").
    pub obligation_bound: f64,
    /// The lower bound established by verified evidence.
    pub evidence_bound: f64,
    /// Which steps provide the evidence.
    pub evidence_steps: Vec<u32>,
    /// Human-readable explanation.
    pub explanation: String,
}

// Regex patterns for extracting numeric facts from natural language.
// These are compiled once and reused.
static RE_FUNC_VALUE: LazyLock<Regex> = LazyLock::new(|| {
    // Matches "f(2) = 4", "f(2)=4", "f(n) = 3n/2" (captures f(digit) = digit)
    Regex::new(r"f\((\d+)\)\s*=\s*(\d+)").unwrap()
});

static RE_RATIO: LazyLock<Regex> = LazyLock::new(|| {
    // Matches "ratio 2.0", "ratio = 2", "ratio f(n)/n = 2", "ratio 3/2"
    Regex::new(r"ratio[^0-9]*(\d+(?:\.\d+)?(?:/\d+)?)").unwrap()
});

static RE_EXCEEDS: LazyLock<Regex> = LazyLock::new(|| {
    // Matches "exceeds 3/2", "exceeds 1.5", "> 3/2"
    Regex::new(r"(?:exceeds|>)\s*(\d+(?:\.\d+)?(?:/\d+)?)").unwrap()
});

static RE_OBLIGATION_BOUND: LazyLock<Regex> = LazyLock::new(|| {
    // Matches "≤ 3/2", "<= 3/2", "≤ (3/2)", "f(n) <= (3/2)n", "c = 3/2"
    Regex::new(r"(?:[≤<=]+|(?:at most))\s*\(?(\d+(?:\.\d+)?(?:/\d+)?)\)?").unwrap()
});

static RE_OBLIGATION_EXACT: LazyLock<Regex> = LazyLock::new(|| {
    // Matches "c = 3/2", "answer is 3/2", "constant is 3/2"
    Regex::new(r"(?:c\s*=|answer\s+is|constant\s+is)\s*(\d+(?:\.\d+)?(?:/\d+)?)").unwrap()
});

/// Parse a fraction string like "3/2" or "1.5" or "4" into f64.
fn parse_numeric(s: &str) -> Option<f64> {
    if let Some((num, den)) = s.split_once('/') {
        let n: f64 = num.trim().parse().ok()?;
        let d: f64 = den.trim().parse().ok()?;
        if d == 0.0 {
            return None;
        }
        Some(n / d)
    } else {
        s.trim().parse().ok()
    }
}

/// Extract numeric evidence facts from the verified chain.
///
/// The chain is: Vec<(step_id, step_number, proposal_type, natural, formal)>
pub fn extract_evidence(verified: &[(String, u32, String, String, String)]) -> Vec<EvidenceFact> {
    let mut facts = Vec::new();

    for (_, step_num, ptype, natural, _formal) in verified {
        // Only look at computation and algebraic steps (where numeric facts live)
        if ptype != "computation" && ptype != "algebraic" && ptype != "lemma" {
            continue;
        }

        // Extract function values: f(n) = k
        for cap in RE_FUNC_VALUE.captures_iter(natural) {
            if let (Some(n_match), Some(v_match)) = (cap.get(1), cap.get(2)) {
                if let (Ok(n), Ok(v)) = (
                    n_match.as_str().parse::<f64>(),
                    v_match.as_str().parse::<f64>(),
                ) {
                    if n > 0.0 {
                        let ratio = v / n;
                        facts.push(EvidenceFact {
                            step_number: *step_num,
                            fact_type: "function_value".into(),
                            summary: format!("f({}) = {} (ratio {:.2})", n as u32, v as u32, ratio),
                            lower_bound: Some(ratio),
                            variable: Some(format!("f({})", n as u32)),
                            value: Some(v),
                        });
                    }
                }
            }
        }

        // Extract explicit ratio mentions
        for cap in RE_RATIO.captures_iter(natural) {
            if let Some(r_match) = cap.get(1) {
                if let Some(ratio) = parse_numeric(r_match.as_str()) {
                    if ratio > 0.0 {
                        facts.push(EvidenceFact {
                            step_number: *step_num,
                            fact_type: "ratio".into(),
                            summary: format!("ratio {} (step {})", r_match.as_str(), step_num),
                            lower_bound: Some(ratio),
                            variable: None,
                            value: Some(ratio),
                        });
                    }
                }
            }
        }

        // Extract "exceeds X" patterns
        for cap in RE_EXCEEDS.captures_iter(natural) {
            if let Some(b_match) = cap.get(1) {
                if let Some(bound) = parse_numeric(b_match.as_str()) {
                    facts.push(EvidenceFact {
                        step_number: *step_num,
                        fact_type: "bound".into(),
                        summary: format!("exceeds {} (step {})", b_match.as_str(), step_num),
                        lower_bound: Some(bound),
                        variable: None,
                        value: Some(bound),
                    });
                }
            }
        }
    }

    // Deduplicate: keep the highest ratio per variable
    facts.sort_by(|a, b| {
        b.lower_bound
            .unwrap_or(0.0)
            .partial_cmp(&a.lower_bound.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    facts
}

/// Find the best (highest) lower bound established by verified evidence.
pub fn best_lower_bound(facts: &[EvidenceFact]) -> Option<f64> {
    facts
        .iter()
        .filter_map(|f| f.lower_bound)
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
}

/// Extract the upper bound claimed by an obligation description.
/// Returns None if no numeric bound is found.
fn extract_obligation_bound(description: &str) -> Option<f64> {
    // Try "≤ X" or "<= X" first
    if let Some(cap) = RE_OBLIGATION_BOUND.captures(description) {
        if let Some(b_match) = cap.get(1) {
            return parse_numeric(b_match.as_str());
        }
    }
    // Try "c = X" or "answer is X"
    if let Some(cap) = RE_OBLIGATION_EXACT.captures(description) {
        if let Some(b_match) = cap.get(1) {
            return parse_numeric(b_match.as_str());
        }
    }
    None
}

/// Check open obligations against verified evidence.
/// Returns conflicts where evidence proves an obligation's bound is wrong.
pub fn find_obligation_conflicts(
    facts: &[EvidenceFact],
    obligations: &[Obligation],
) -> Vec<ObligationConflict> {
    let mut conflicts = Vec::new();

    let best_lb = match best_lower_bound(facts) {
        Some(b) if b > 0.0 => b,
        _ => return conflicts,
    };

    // Collect step numbers that contribute to the best bound
    let evidence_steps: Vec<u32> = facts
        .iter()
        .filter(|f| f.lower_bound.map(|b| b >= best_lb - 0.001).unwrap_or(false))
        .map(|f| f.step_number)
        .collect();

    for ob in obligations {
        // Skip already-closed obligations
        if ob.status != "open" {
            continue;
        }

        let desc = &ob.description;

        // Extract the bound the obligation claims
        if let Some(ob_bound) = extract_obligation_bound(desc) {
            // Conflict: evidence shows lower bound > obligation's upper bound
            if best_lb > ob_bound + 0.001 {
                conflicts.push(ObligationConflict {
                    obligation_id: ob.id.clone(),
                    obligation_desc: desc.clone(),
                    obligation_bound: ob_bound,
                    evidence_bound: best_lb,
                    evidence_steps: evidence_steps.clone(),
                    explanation: format!(
                        "Verified evidence establishes a lower bound of {:.2}, \
                         which contradicts this obligation's claim of ≤ {:.2}. \
                         The evidence comes from {} verified computation steps.",
                        best_lb,
                        ob_bound,
                        evidence_steps.len()
                    ),
                });
            }
        }

        // Also check obligations that reference a specific c value
        // e.g., "conclude that the smallest constant is c = 3/2"
        let desc_lower = desc.to_lowercase();
        if desc_lower.contains("c = ")
            || desc_lower.contains("c=")
            || desc_lower.contains("constant")
            || desc_lower.contains("smallest")
        {
            if let Some(cap) = RE_OBLIGATION_EXACT.captures(desc) {
                if let Some(v_match) = cap.get(1) {
                    if let Some(claimed_c) = parse_numeric(v_match.as_str()) {
                        if best_lb > claimed_c + 0.001 {
                            // Avoid duplicate if already caught by bound check
                            if !conflicts.iter().any(|c| c.obligation_id == ob.id) {
                                conflicts.push(ObligationConflict {
                                    obligation_id: ob.id.clone(),
                                    obligation_desc: desc.clone(),
                                    obligation_bound: claimed_c,
                                    evidence_bound: best_lb,
                                    evidence_steps: evidence_steps.clone(),
                                    explanation: format!(
                                        "This obligation claims c = {:.2}, but verified evidence \
                                         shows c >= {:.2} from construction steps {:?}.",
                                        claimed_c, best_lb, evidence_steps
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    conflicts
}

/// Check if a proposed obligation description conflicts with verified evidence.
/// Returns Some(explanation) if it conflicts, None if it's compatible.
/// Used to gate audit obligation creation — prevent contaminated obligations
/// from entering the DB in the first place.
pub fn proposal_conflicts_with_evidence(
    facts: &[EvidenceFact],
    description: &str,
) -> Option<String> {
    let best_lb = match best_lower_bound(facts) {
        Some(b) if b > 0.0 => b,
        _ => return None,
    };

    // Check if the proposed obligation asserts an upper bound below our evidence
    if let Some(ob_bound) = extract_obligation_bound(description) {
        if best_lb > ob_bound + 0.001 {
            return Some(format!(
                "Proposed obligation claims bound {:.4} but verified evidence shows c >= {:.4}",
                ob_bound, best_lb
            ));
        }
    }

    // Check "c = X" style claims
    if let Some(cap) = RE_OBLIGATION_EXACT.captures(description) {
        if let Some(v_match) = cap.get(1) {
            if let Some(claimed_c) = parse_numeric(v_match.as_str()) {
                if best_lb > claimed_c + 0.001 {
                    return Some(format!(
                        "Proposed obligation claims c = {:.4} but verified evidence shows c >= {:.4}",
                        claimed_c, best_lb
                    ));
                }
            }
        }
    }

    None
}

/// Build a compact evidence summary for injection into the solver prompt.
/// This tells the solver what its own verified work has established.
pub fn build_evidence_summary(facts: &[EvidenceFact]) -> String {
    if facts.is_empty() {
        return String::new();
    }

    let best_lb = best_lower_bound(facts);

    let mut summary = String::from(
        "EVIDENCE ESTABLISHED BY YOUR VERIFIED WORK (these are PROVEN facts, not claims):\n",
    );

    // Show unique function values with highest ratios
    let mut seen_vars: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut shown = 0;
    for f in facts {
        if shown >= 6 {
            break;
        }
        let key = f.variable.as_deref().unwrap_or(&f.summary).to_string();
        if seen_vars.contains(&key) {
            continue;
        }
        seen_vars.insert(key);
        summary.push_str(&format!("  - {} [step {}]\n", f.summary, f.step_number));
        shown += 1;
    }

    if let Some(lb) = best_lb {
        summary.push_str(&format!(
            "  >> ESTABLISHED LOWER BOUND: c >= {:.4} (from verified constructions)\n",
            lb
        ));
        summary.push_str(
            "  >> Any conclusion claiming c < this value CONTRADICTS your own verified work.\n",
        );
    }

    summary
}

/// Build a conflict warning for injection into the solver prompt when
/// obligations contradict verified evidence.
pub fn build_conflict_warning(conflicts: &[ObligationConflict]) -> String {
    if conflicts.is_empty() {
        return String::new();
    }

    let mut warning = String::from(
        "WARNING — EVIDENCE-OBLIGATION CONFLICTS DETECTED:\n\
         The following obligations appear to contradict your verified computations.\n\
         This may indicate the obligations are based on an incorrect training prior.\n\
         Your verified evidence takes precedence over unverified claims.\n\n",
    );

    for c in conflicts.iter().take(5) {
        warning.push_str(&format!(
            "  CONFLICT: Obligation claims c <= {:.4}, but verified evidence shows c >= {:.4}\n",
            c.obligation_bound, c.evidence_bound
        ));
        warning.push_str(&format!(
            "    Obligation: \"{}\"\n",
            truncate(&c.obligation_desc, 120)
        ));
        warning.push_str(&format!(
            "    Evidence: {} verified steps (steps {:?})\n\n",
            c.evidence_steps.len(),
            c.evidence_steps
        ));
    }

    warning.push_str(
        "  ACTION: These obligations should be CLOSED as invalidated, not resolved.\n\
         If you are concluding, state the answer supported by verified evidence, not by prior belief.\n"
    );

    warning
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_step(
        n: u32,
        ptype: &str,
        natural: &str,
        formal: &str,
    ) -> (String, u32, String, String, String) {
        (
            format!("step-{}", n),
            n,
            ptype.into(),
            natural.into(),
            formal.into(),
        )
    }

    #[test]
    fn extracts_function_values() {
        let chain = vec![
            make_step(
                21,
                "computation",
                "We check f(2)=4 is bonza because 4 | -252",
                "2**2 - 4**4 = -252",
            ),
            make_step(
                22,
                "computation",
                "We check f(4)=8: need 8 | -65520",
                "16 - 4**8 = -65520",
            ),
        ];
        let facts = extract_evidence(&chain);
        assert!(!facts.is_empty());
        let best = best_lower_bound(&facts);
        assert!(best.is_some());
        assert!(
            best.unwrap() >= 2.0,
            "f(2)=4 gives ratio 2.0, got {}",
            best.unwrap()
        );
    }

    fn make_obligation(id: &str, desc: &str) -> Obligation {
        Obligation {
            id: id.into(),
            attempt_id: "att-1".into(),
            branch_id: 0,
            parent_node_id: String::new(),
            description: desc.into(),
            obligation_type: "proof".into(),
            priority: 1.0,
            confidence: 0.8,
            source_layer: None,
            status: "open".into(),
            assigned_model: None,
            closure_node_id: None,
            closure_type: None,
            escalation_level: 0,
            steps_spent: 0,
            max_steps: 10,
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
            active_solver_round_id: None,
            assigned_models_json: None,
            created_at: String::new(),
            closed_at: None,
        }
    }

    #[test]
    fn detects_obligation_conflict() {
        let chain = vec![make_step(
            21,
            "computation",
            "f(2)=4 satisfies bonza, ratio f(2)/2=2 exceeds 3/2",
            "2**2 - 4**4 = -252",
        )];
        let facts = extract_evidence(&chain);

        let obligations = vec![make_obligation(
            "ob-1",
            "Prove that for any bonza function f, f(n) <= (3/2)n for all n",
        )];

        let conflicts = find_obligation_conflicts(&facts, &obligations);
        assert!(
            !conflicts.is_empty(),
            "Should detect conflict between ratio 2.0 and bound 3/2"
        );
        assert!(conflicts[0].evidence_bound >= 2.0);
        assert!((conflicts[0].obligation_bound - 1.5).abs() < 0.01);
    }

    #[test]
    fn no_conflict_when_evidence_below_bound() {
        let chain = vec![make_step(
            5,
            "computation",
            "f(2)=2 satisfies bonza",
            "2**2 - 2**2 = 0",
        )];
        let facts = extract_evidence(&chain);
        let obligations = vec![make_obligation("ob-1", "Prove that f(n) <= 2n for all n")];
        let conflicts = find_obligation_conflicts(&facts, &obligations);
        assert!(
            conflicts.is_empty(),
            "ratio 1.0 does not conflict with bound 2.0"
        );
    }
}
