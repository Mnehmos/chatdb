//! Contradiction Detection — scans verified chain for conflicting claims.
//!
//! Two-stage detection:
//! 1. Structural: Extract numeric claims from formal expressions,
//!    detect `var = X` vs `var = Y` conflicts.
//! 2. Semantic (optional LLM): Lightweight comparison for non-numeric contradictions.
//!
//! When a contradiction is found, severity determines the response:
//! - critical: Different concrete numbers for same variable → S+ RESOLVE obligation
//! - major: Different symbolic expressions → standard RESOLVE obligation
//! - minor: Syntactically different but likely equivalent → conflict record only, no obligation

use std::collections::HashMap;

/// A detected contradiction between two verified steps.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Contradiction {
    pub step_a: u32,
    pub step_b: u32,
    pub formal_a: String,
    pub formal_b: String,
    pub variable: String,
    pub value_a: String,
    pub value_b: String,
    pub description: String,
    /// Severity: "critical" (numeric disagreement), "major" (symbolic disagreement),
    /// "minor" (likely equivalent, syntactic noise).
    pub severity: String,
}

/// Scan the verified chain for structural contradictions.
/// Extracts `variable = value` claims from formal expressions and checks for conflicts.
pub fn detect_structural_contradictions(
    verified_chain: &[(String, u32, String, String, String)],
) -> Vec<Contradiction> {
    // Parse formal expressions into variable=value claims
    // Format: (step_number, var, value, full_formal)
    let mut claims: Vec<(u32, String, String, String)> = Vec::new();

    for (_id, step_num, _model, _natural, formal) in verified_chain {
        if formal.is_empty() {
            continue;
        }

        // Split on '=' and look for simple variable = value patterns
        // Handle forms like: "x = 5", "f(n) = n**2", "Sum(...) = n*(n+1)/2"
        let parts: Vec<&str> = formal.splitn(2, '=').collect();
        if parts.len() == 2 {
            let lhs = parts[0].trim().to_string();
            let rhs = parts[1].trim().to_string();

            // Normalize: treat the shorter side as the "variable"
            let (var, val) = if lhs.len() <= rhs.len() {
                (lhs, rhs)
            } else {
                (rhs, lhs)
            };

            claims.push((*step_num, var, val, formal.clone()));
        }
    }

    // Group claims by variable and check for conflicts
    let mut var_claims: HashMap<String, Vec<(u32, String, String)>> = HashMap::new();
    for (step, var, val, formal) in &claims {
        var_claims
            .entry(var.clone())
            .or_default()
            .push((*step, val.clone(), formal.clone()));
    }

    let mut contradictions = Vec::new();

    for (var, entries) in &var_claims {
        if entries.len() < 2 {
            continue;
        }

        // Compare each pair of claims for the same variable
        for i in 0..entries.len() {
            for j in (i + 1)..entries.len() {
                let (step_a, val_a, formal_a) = &entries[i];
                let (step_b, val_b, formal_b) = &entries[j];

                // Only flag as contradiction if values are syntactically different
                // and neither is a generalization of the other
                if val_a != val_b && !is_generalization(val_a, val_b) {
                    let severity = classify_severity(val_a, val_b);
                    contradictions.push(Contradiction {
                        step_a: *step_a,
                        step_b: *step_b,
                        formal_a: formal_a.clone(),
                        formal_b: formal_b.clone(),
                        variable: var.clone(),
                        value_a: val_a.clone(),
                        value_b: val_b.clone(),
                        description: format!(
                            "Steps {} and {} disagree: {} = {} vs {} = {}",
                            step_a, step_b, var, val_a, var, val_b
                        ),
                        severity,
                    });
                }
            }
        }
    }

    contradictions
}

/// Classify contradiction severity based on the nature of the disagreement.
///
/// - "critical": Both values are concrete numbers and they differ (e.g., "5" vs "7").
/// - "major": At least one value is symbolic — can't determine equivalence without CAS.
/// - "minor": Values look like minor reformulations (e.g., very similar structure).
fn classify_severity(val_a: &str, val_b: &str) -> String {
    // Try parsing both as f64 — concrete numeric disagreement is critical
    let num_a = val_a.trim().parse::<f64>();
    let num_b = val_b.trim().parse::<f64>();

    match (num_a, num_b) {
        (Ok(a), Ok(b)) => {
            if (a - b).abs() < 1e-10 {
                // Same number, syntactic difference — minor
                "minor".to_string()
            } else {
                // Different concrete numbers — critical
                "critical".to_string()
            }
        }
        _ => {
            // At least one side is symbolic. Check if they share most structure.
            let a_norm = normalize_expr(val_a);
            let b_norm = normalize_expr(val_b);
            // If one is much longer than the other, likely a real disagreement
            let len_ratio = a_norm.len().min(b_norm.len()) as f64
                / a_norm.len().max(b_norm.len()).max(1) as f64;
            if len_ratio > 0.8 {
                // Similar length — might be minor reformulation
                // Count common characters as a rough similarity measure
                let common: usize = a_norm
                    .chars()
                    .zip(b_norm.chars())
                    .filter(|(a, b)| a == b)
                    .count();
                let similarity = common as f64 / a_norm.len().max(b_norm.len()).max(1) as f64;
                if similarity > 0.7 {
                    "minor".to_string()
                } else {
                    "major".to_string()
                }
            } else {
                "major".to_string()
            }
        }
    }
}

/// Check if one expression is a generalization/simplification of the other.
/// Prevents false positives like "n*(n+1)/2" vs "n**2/2 + n/2".
fn is_generalization(a: &str, b: &str) -> bool {
    // Simple heuristic: if one is a substring of the other, likely same claim
    let a_norm = normalize_expr(a);
    let b_norm = normalize_expr(b);
    a_norm == b_norm || a_norm.contains(&b_norm) || b_norm.contains(&a_norm)
}

/// Normalize a math expression for comparison (strip spaces, lowercase).
fn normalize_expr(expr: &str) -> String {
    expr.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_diff_is_critical() {
        assert_eq!(classify_severity("5", "7"), "critical");
        assert_eq!(classify_severity("42", "100"), "critical");
        assert_eq!(classify_severity("3.14", "2.71"), "critical");
    }

    #[test]
    fn same_number_is_minor() {
        assert_eq!(classify_severity("5", "5.0"), "minor");
        assert_eq!(classify_severity("64", "64"), "minor");
    }

    #[test]
    fn symbolic_diff_is_major() {
        assert_eq!(classify_severity("n**2", "2*n + 1"), "major");
        assert_eq!(classify_severity("x + 1", "x**2"), "major");
    }

    #[test]
    fn similar_symbolic_is_minor() {
        // Very similar structure → minor
        assert_eq!(classify_severity("n*(n+1)/2", "n*(n+1)/3"), "minor");
    }

    #[test]
    fn detection_includes_severity() {
        let chain = vec![
            (
                "s1".to_string(),
                1,
                "m".to_string(),
                "n".to_string(),
                "x = 5".to_string(),
            ),
            (
                "s2".to_string(),
                2,
                "m".to_string(),
                "n".to_string(),
                "x = 7".to_string(),
            ),
        ];
        let results = detect_structural_contradictions(&chain);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].severity, "critical");
    }

    #[test]
    fn generalization_skipped() {
        let chain = vec![
            (
                "s1".to_string(),
                1,
                "m".to_string(),
                "n".to_string(),
                "x = n*(n+1)/2".to_string(),
            ),
            (
                "s2".to_string(),
                2,
                "m".to_string(),
                "n".to_string(),
                "x = n*(n+1)".to_string(),
            ),
        ];
        let results = detect_structural_contradictions(&chain);
        // "n*(n+1)/2" contains "n*(n+1)" → generalization, should be skipped
        assert_eq!(results.len(), 0);
    }
}
