//! Satisfaction Criteria — type-specific checks for when an obligation is "done."
//!
//! Three-stage resolution pipeline (replaces batched LLM-only check):
//! 1. Tautology filter: instant substring/formal match — close without LLM
//! 2. Type-specific satisfaction: structural check per obligation type
//! 3. LLM fallback: existing batch prompt for ambiguous cases only
//!
//! Cumulative obligation checking: when single-step checks return Ambiguous,
//! the pipeline falls back to scanning all verified ProofNodes that reference
//! the obligation, looking for collective satisfaction across multiple steps.

use std::collections::HashSet;

use crate::models::dag::{Obligation, ProofNode};

/// Result of checking whether a verified step satisfies an obligation.
#[derive(Debug)]
pub enum SatisfactionResult {
    /// Obligation is clearly satisfied by this step — close it.
    Satisfied { note: String },
    /// Obligation is clearly NOT satisfied — skip LLM check.
    NotSatisfied,
    /// Ambiguous — needs LLM to decide.
    Ambiguous,
}

// ---------------------------------------------------------------------------
// Helper functions for cumulative obligation checking
// ---------------------------------------------------------------------------

/// Filter obligation nodes to only those with status "verified".
fn verified_nodes(nodes: &[ProofNode]) -> Vec<&ProofNode> {
    nodes.iter().filter(|n| n.status == "verified").collect()
}

/// Concatenate the natural-language content of all verified nodes into one string.
fn cumulative_text(nodes: &[ProofNode]) -> String {
    nodes
        .iter()
        .filter(|n| n.status == "verified")
        .map(|n| n.content.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Collect formal_content from all verified nodes into a single vec of slices.
fn cumulative_formals(nodes: &[ProofNode]) -> Vec<&str> {
    nodes
        .iter()
        .filter(|n| n.status == "verified")
        .filter_map(|n| n.formal_content.as_deref())
        .collect()
}

// ---------------------------------------------------------------------------
// Stage 1: Tautology filter
// ---------------------------------------------------------------------------

/// Stage 1: Tautology filter — instant check.
/// If the obligation's description is a substring of the step's natural text,
/// or the formal expression matches, close immediately.
///
/// Cumulative fallback: if single-step check is ambiguous, check whether the
/// combined verified obligation nodes collectively match at >= 90% keyword
/// overlap with at least one formal expression present.
pub fn tautology_check(
    obligation: &Obligation,
    step_natural: &str,
    step_formal: Option<&str>,
    obligation_nodes: &[ProofNode],
) -> SatisfactionResult {
    let desc_lower = obligation.description.to_lowercase();
    let step_lower = step_natural.to_lowercase();

    // Check if the step's natural language directly addresses the obligation
    // by checking for high keyword overlap
    let desc_words: Vec<&str> = desc_lower
        .split_whitespace()
        .filter(|w| w.len() > 3) // skip small words
        .collect();

    if desc_words.is_empty() {
        return SatisfactionResult::Ambiguous;
    }

    let matching = desc_words
        .iter()
        .filter(|w| step_lower.contains(**w))
        .count();

    let overlap = matching as f64 / desc_words.len() as f64;

    // If >80% of significant words match AND there's a formal expression, likely satisfied
    if overlap >= 0.8 && step_formal.is_some() {
        return SatisfactionResult::Satisfied {
            note: format!(
                "Tautology: step content matches obligation ({:.0}% keyword overlap)",
                overlap * 100.0
            ),
        };
    }

    // Check satisfaction_criteria match if available
    if let Some(ref criteria) = obligation.satisfaction_criteria {
        let criteria_lower = criteria.to_lowercase();
        if let Some(formal) = step_formal {
            let formal_lower = formal.to_lowercase();
            // If the formal expression contains key terms from satisfaction criteria
            let criteria_words: Vec<&str> = criteria_lower
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .collect();
            if !criteria_words.is_empty() {
                let criteria_match = criteria_words
                    .iter()
                    .filter(|w| formal_lower.contains(**w) || step_lower.contains(**w))
                    .count();
                let criteria_overlap = criteria_match as f64 / criteria_words.len() as f64;
                if criteria_overlap >= 0.7 {
                    return SatisfactionResult::Satisfied {
                        note: format!(
                            "Criteria match: {:.0}% overlap with satisfaction criteria",
                            criteria_overlap * 100.0
                        ),
                    };
                }
            }
        }
    }

    // --- Cumulative fallback ---
    let v_nodes = verified_nodes(obligation_nodes);
    if !v_nodes.is_empty() {
        let cum_text = cumulative_text(obligation_nodes).to_lowercase();
        let cum_matching = desc_words.iter().filter(|w| cum_text.contains(**w)).count();
        let cum_overlap = cum_matching as f64 / desc_words.len() as f64;
        let has_any_formal = v_nodes.iter().any(|n| n.formal_content.is_some());

        if cum_overlap >= 0.9 && has_any_formal {
            return SatisfactionResult::Satisfied {
                note: format!(
                    "Tautology (cumulative): {} verified nodes collectively match obligation ({:.0}% keyword overlap)",
                    v_nodes.len(), cum_overlap * 100.0
                ),
            };
        }

        // Also check satisfaction_criteria against cumulative formals
        if let Some(ref criteria) = obligation.satisfaction_criteria {
            let criteria_lower = criteria.to_lowercase();
            let criteria_words: Vec<&str> = criteria_lower
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .collect();
            if !criteria_words.is_empty() {
                let formals = cumulative_formals(obligation_nodes);
                let formals_combined: String = formals
                    .iter()
                    .map(|f| f.to_lowercase())
                    .collect::<Vec<_>>()
                    .join(" ");
                let criteria_match = criteria_words
                    .iter()
                    .filter(|w| formals_combined.contains(**w) || cum_text.contains(**w))
                    .count();
                let criteria_overlap = criteria_match as f64 / criteria_words.len() as f64;
                if criteria_overlap >= 0.7 && formals.len() >= 2 {
                    return SatisfactionResult::Satisfied {
                        note: format!(
                            "Criteria match (cumulative): {:.0}% overlap across {} formal expressions",
                            criteria_overlap * 100.0, formals.len()
                        ),
                    };
                }
            }
        }
    }

    SatisfactionResult::Ambiguous
}

// ---------------------------------------------------------------------------
// Stage 2: Type-specific structural satisfaction checks
// ---------------------------------------------------------------------------

/// Stage 2: Type-specific structural satisfaction check.
/// Uses the obligation type to apply domain-appropriate checks.
pub fn type_specific_check(
    obligation: &Obligation,
    step_natural: &str,
    step_formal: Option<&str>,
    verified_chain: &[(String, u32, String, String, String)],
    obligation_nodes: &[ProofNode],
) -> SatisfactionResult {
    match obligation.obligation_type.to_uppercase().as_str() {
        "COUNT" => check_count(obligation, step_natural, step_formal, obligation_nodes),
        "BOUND" => check_bound(obligation, step_natural, step_formal, obligation_nodes),
        "CASE_CHECK" | "CONSTRAINT_CHECK" => {
            check_case(obligation, step_natural, step_formal, obligation_nodes)
        }
        "CONSTRUCT" | "CONSTRUCTION" => {
            check_construct(obligation, step_natural, step_formal, obligation_nodes)
        }
        "CLASSIFY" => check_classify(obligation, step_natural, verified_chain, obligation_nodes),
        "IMPOSSIBILITY" => check_impossibility(obligation, step_natural, obligation_nodes),
        "SYNTHESIZE" => check_synthesize(obligation, verified_chain, obligation_nodes),
        _ => SatisfactionResult::Ambiguous,
    }
}

/// COUNT: Verified step establishes a claimed equality involving a quantity.
fn check_count(
    obligation: &Obligation,
    step_natural: &str,
    step_formal: Option<&str>,
    obligation_nodes: &[ProofNode],
) -> SatisfactionResult {
    let desc = obligation.description.to_lowercase();

    // Look for numeric/quantity terms in both obligation and step
    let quantity_terms = [
        "count",
        "number",
        "total",
        "how many",
        "cardinality",
        "size",
    ];
    let has_quantity = quantity_terms.iter().any(|t| desc.contains(t));

    if has_quantity {
        if let Some(formal) = step_formal {
            // If formal expression contains an = and relates to counting
            if formal.contains('=') {
                let step_lower = step_natural.to_lowercase();
                let overlap = quantity_terms.iter().any(|t| step_lower.contains(t));
                if overlap {
                    return SatisfactionResult::Satisfied {
                        note: "COUNT: verified equality establishes the claimed quantity".into(),
                    };
                }
            }
        }
    }

    // --- Cumulative fallback ---
    let v_nodes = verified_nodes(obligation_nodes);
    if v_nodes.len() >= 2 && has_quantity {
        let formals = cumulative_formals(obligation_nodes);
        let has_formal_equality = formals.iter().any(|f| f.contains('='));
        let cum_text = cumulative_text(obligation_nodes).to_lowercase();
        let has_quantity_term = quantity_terms.iter().any(|t| cum_text.contains(t));
        if has_formal_equality && has_quantity_term {
            return SatisfactionResult::Satisfied {
                note: format!(
                    "COUNT (cumulative): {} verified nodes collectively establish the quantity",
                    v_nodes.len()
                ),
            };
        }
    }

    SatisfactionResult::Ambiguous
}

/// BOUND: Step establishes an inequality or extremal result.
fn check_bound(
    obligation: &Obligation,
    step_natural: &str,
    step_formal: Option<&str>,
    obligation_nodes: &[ProofNode],
) -> SatisfactionResult {
    let desc = obligation.description.to_lowercase();
    let step_lower = step_natural.to_lowercase();

    let bound_terms = [
        "maximum",
        "minimum",
        "at most",
        "at least",
        "upper bound",
        "lower bound",
        "greatest",
        "least",
    ];
    let desc_has_bound = bound_terms.iter().any(|t| desc.contains(t));
    let step_has_bound = bound_terms.iter().any(|t| step_lower.contains(t));

    if desc_has_bound && step_has_bound && step_formal.is_some() {
        return SatisfactionResult::Satisfied {
            note: "BOUND: step establishes the required bound".into(),
        };
    }

    // --- Cumulative fallback ---
    let v_nodes = verified_nodes(obligation_nodes);
    if v_nodes.len() >= 2 && desc_has_bound {
        let cum_text = cumulative_text(obligation_nodes).to_lowercase();
        let cum_has_bound = bound_terms.iter().any(|t| cum_text.contains(t));
        let formals = cumulative_formals(obligation_nodes);
        if cum_has_bound && !formals.is_empty() {
            return SatisfactionResult::Satisfied {
                note: format!(
                    "BOUND (cumulative): {} verified nodes collectively establish the bound",
                    v_nodes.len()
                ),
            };
        }
    }

    SatisfactionResult::Ambiguous
}

/// CASE_CHECK: All specific instances verified.
fn check_case(
    obligation: &Obligation,
    step_natural: &str,
    step_formal: Option<&str>,
    obligation_nodes: &[ProofNode],
) -> SatisfactionResult {
    let desc = obligation.description.to_lowercase();
    let step_lower = step_natural.to_lowercase();

    // Check if the step explicitly addresses the case mentioned in the obligation
    let case_terms = [
        "case",
        "check",
        "verify",
        "when",
        "for n=",
        "for k=",
        "base case",
    ];
    let desc_case = case_terms.iter().any(|t| desc.contains(t));
    let step_case = case_terms.iter().any(|t| step_lower.contains(t));

    if desc_case && step_case && step_formal.is_some() {
        return SatisfactionResult::Satisfied {
            note: "CASE_CHECK: step verifies the required case".into(),
        };
    }

    // --- Cumulative fallback ---
    let v_nodes = verified_nodes(obligation_nodes);
    if v_nodes.len() >= 2 && desc_case {
        let cum_text = cumulative_text(obligation_nodes).to_lowercase();
        let cum_has_case = case_terms.iter().any(|t| cum_text.contains(t));
        let formals = cumulative_formals(obligation_nodes);
        if cum_has_case && !formals.is_empty() {
            return SatisfactionResult::Satisfied {
                note: format!(
                    "CASE_CHECK (cumulative): {} verified nodes collectively cover the case",
                    v_nodes.len()
                ),
            };
        }
    }

    SatisfactionResult::Ambiguous
}

/// CONSTRUCT: Step provides explicit example with formal verification.
///
/// For competition math, CONSTRUCT means "produce a verified algebraic chain" —
/// the obligation description rarely contains the word "construct".
/// Primary path: ≥ 2 verified obligation nodes with formal expressions = satisfied.
/// Keyword path (legacy): description AND step both contain construct vocabulary.
fn check_construct(
    obligation: &Obligation,
    step_natural: &str,
    step_formal: Option<&str>,
    obligation_nodes: &[ProofNode],
) -> SatisfactionResult {
    // --- Algebraic chain path (primary for competition math) ---
    // A CONSTRUCT obligation is satisfied when the cumulative chain has produced
    // ≥ 2 verified steps with formal expressions targeting this obligation.
    // This is the "constructed proof" — a chain of verified identities.
    let v_nodes = verified_nodes(obligation_nodes);
    let formals = cumulative_formals(obligation_nodes);
    if v_nodes.len() >= 2 && formals.len() >= 2 {
        return SatisfactionResult::Satisfied {
            note: format!(
                "CONSTRUCT: {} verified nodes with {} formal expressions form the required construction",
                v_nodes.len(), formals.len()
            ),
        };
    }

    // --- Keyword path (for explicit "construct/exhibit" style obligations) ---
    let desc = obligation.description.to_lowercase();
    let step_lower = step_natural.to_lowercase();

    let construct_terms = [
        "construct",
        "example",
        "configuration",
        "arrangement",
        "exhibit",
    ];
    let desc_has = construct_terms.iter().any(|t| desc.contains(t));
    let step_has = construct_terms.iter().any(|t| step_lower.contains(t));

    if desc_has && step_has && step_formal.is_some() {
        return SatisfactionResult::Satisfied {
            note: "CONSTRUCT: step provides and verifies the required construction".into(),
        };
    }

    // --- Cumulative keyword fallback ---
    if v_nodes.len() >= 2 && desc_has {
        let cum_text = cumulative_text(obligation_nodes).to_lowercase();
        let cum_has_construct = construct_terms.iter().any(|t| cum_text.contains(t));
        if cum_has_construct && !formals.is_empty() {
            return SatisfactionResult::Satisfied {
                note: format!(
                    "CONSTRUCT (cumulative): {} verified nodes collectively provide the construction",
                    v_nodes.len()
                ),
            };
        }
    }

    SatisfactionResult::Ambiguous
}

/// CLASSIFY: Verified steps cover all cases.
/// Scans obligation nodes for case markers and exhaustion claims.
fn check_classify(
    obligation: &Obligation,
    _step_natural: &str,
    _verified_chain: &[(String, u32, String, String, String)],
    obligation_nodes: &[ProofNode],
) -> SatisfactionResult {
    let _ = obligation;

    let v_nodes = verified_nodes(obligation_nodes);
    if v_nodes.is_empty() {
        return SatisfactionResult::Ambiguous;
    }

    // Case markers to detect distinct cases
    let case_markers: &[&str] = &[
        "case 1",
        "case 2",
        "case 3",
        "case i",
        "case ii",
        "case iii",
        "first case",
        "second case",
        "third case",
        "subcase",
        "otherwise",
        "remaining case",
        "final case",
    ];

    // Exhaustion claim phrases
    let exhaustion_claims: &[&str] = &[
        "all cases",
        "exhaustive",
        "covers all",
        "complete classification",
    ];

    let mut distinct_cases: HashSet<String> = HashSet::new();
    let mut has_exhaustion = false;

    for node in &v_nodes {
        let content_lower = node.content.to_lowercase();

        // Track distinct case markers found
        for marker in case_markers {
            if content_lower.contains(marker) {
                distinct_cases.insert(marker.to_string());
            }
        }

        // Check for exhaustion claims
        if exhaustion_claims
            .iter()
            .any(|claim| content_lower.contains(claim))
        {
            has_exhaustion = true;
        }
    }

    let case_count = distinct_cases.len();

    // Satisfied when: 2+ distinct cases AND exhaustion claim, OR 3+ distinct cases
    if (case_count >= 2 && has_exhaustion) || case_count >= 3 {
        return SatisfactionResult::Satisfied {
            note: format!(
                "CLASSIFY: {} distinct cases found across {} verified nodes{}",
                case_count,
                v_nodes.len(),
                if has_exhaustion {
                    " with exhaustion claim"
                } else {
                    ""
                },
            ),
        };
    }

    SatisfactionResult::Ambiguous
}

/// IMPOSSIBILITY: Step reaches contradiction from assumption.
fn check_impossibility(
    obligation: &Obligation,
    step_natural: &str,
    obligation_nodes: &[ProofNode],
) -> SatisfactionResult {
    let step_lower = step_natural.to_lowercase();
    let contradiction_terms = [
        "contradiction",
        "impossible",
        "cannot",
        "no such",
        "absurd",
        "violates",
    ];
    let has_contradiction = contradiction_terms.iter().any(|t| step_lower.contains(t));

    if has_contradiction {
        let desc_lower = obligation.description.to_lowercase();
        // Check if the impossibility relates to the obligation's subject
        let desc_words: Vec<&str> = desc_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();
        let matching = desc_words
            .iter()
            .filter(|w| step_lower.contains(**w))
            .count();
        if !desc_words.is_empty() && (matching as f64 / desc_words.len() as f64) > 0.5 {
            return SatisfactionResult::Satisfied {
                note: "IMPOSSIBILITY: step derives the required contradiction".into(),
            };
        }
    }

    // --- Cumulative fallback ---
    let v_nodes = verified_nodes(obligation_nodes);
    if v_nodes.len() >= 2 {
        let cum_text = cumulative_text(obligation_nodes).to_lowercase();
        let cum_has_contradiction = contradiction_terms.iter().any(|t| cum_text.contains(t));
        if cum_has_contradiction {
            let desc_lower = obligation.description.to_lowercase();
            let desc_words: Vec<&str> = desc_lower
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .collect();
            if !desc_words.is_empty() {
                let cum_matching = desc_words.iter().filter(|w| cum_text.contains(**w)).count();
                if (cum_matching as f64 / desc_words.len() as f64) > 0.5 {
                    return SatisfactionResult::Satisfied {
                        note: format!(
                            "IMPOSSIBILITY (cumulative): {} verified nodes collectively derive the contradiction",
                            v_nodes.len()
                        ),
                    };
                }
            }
        }
    }

    SatisfactionResult::Ambiguous
}

/// SYNTHESIZE: Coherent final statement citing all required steps.
/// Requires 2+ verified prior nodes and synthesis language in the current step.
fn check_synthesize(
    obligation: &Obligation,
    _verified_chain: &[(String, u32, String, String, String)],
    obligation_nodes: &[ProofNode],
) -> SatisfactionResult {
    let v_nodes = verified_nodes(obligation_nodes);

    // Need at least 2 verified prior nodes as prerequisites
    if v_nodes.len() < 2 {
        return SatisfactionResult::Ambiguous;
    }

    // Synthesis language markers: the most recent verified node should tie things together
    let synthesis_terms: &[&str] = &[
        "therefore",
        "combining",
        "thus we conclude",
        "putting it all together",
        "from the above",
        "we have shown",
        "this completes",
        "by combining",
        "synthesizing",
        "hence",
        "it follows that",
    ];

    // Check the last verified node for synthesis language (it's the current step
    // or the most recent contribution to this obligation)
    if let Some(last_node) = v_nodes.last() {
        let content_lower = last_node.content.to_lowercase();
        let has_synthesis = synthesis_terms.iter().any(|t| content_lower.contains(t));

        if has_synthesis {
            // Check keyword overlap between obligation description and the synthesis step
            let desc_lower = obligation.description.to_lowercase();
            let desc_words: Vec<&str> = desc_lower
                .split_whitespace()
                .filter(|w| w.len() > 3)
                .collect();

            if !desc_words.is_empty() {
                let matching = desc_words
                    .iter()
                    .filter(|w| content_lower.contains(**w))
                    .count();
                let overlap = matching as f64 / desc_words.len() as f64;

                if overlap > 0.5 {
                    return SatisfactionResult::Satisfied {
                        note: format!(
                            "SYNTHESIZE: synthesis language found in final node with {:.0}% obligation overlap, {} prerequisite nodes",
                            overlap * 100.0, v_nodes.len() - 1
                        ),
                    };
                }
            }
        }
    }

    SatisfactionResult::Ambiguous
}

// ---------------------------------------------------------------------------
// Entry point: three-stage resolution pipeline
// ---------------------------------------------------------------------------

/// Run the full three-stage resolution pipeline for a single obligation.
/// Returns Some(note) if satisfied, None if not.
pub fn check_obligation_satisfaction(
    obligation: &Obligation,
    step_natural: &str,
    step_formal: Option<&str>,
    verified_chain: &[(String, u32, String, String, String)],
    obligation_nodes: &[ProofNode],
) -> SatisfactionResult {
    // Stage 1: Tautology filter
    let tautology = tautology_check(obligation, step_natural, step_formal, obligation_nodes);
    if matches!(
        tautology,
        SatisfactionResult::Satisfied { .. } | SatisfactionResult::NotSatisfied
    ) {
        return tautology;
    }

    // Stage 2: Type-specific check
    let type_check = type_specific_check(
        obligation,
        step_natural,
        step_formal,
        verified_chain,
        obligation_nodes,
    );
    if matches!(
        type_check,
        SatisfactionResult::Satisfied { .. } | SatisfactionResult::NotSatisfied
    ) {
        return type_check;
    }

    // Stage 3: Ambiguous — caller should use LLM fallback
    SatisfactionResult::Ambiguous
}
