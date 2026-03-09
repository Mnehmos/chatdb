//! Obligation Queue — drives solver step selection.
//!
//! Instead of the solver deciding what to work on next (freeform chain extension),
//! the queue selects the highest-priority unblocked obligation and assigns the
//! solver to it. When no obligations exist, falls back to freeform mode.

use crate::db::Database;
use crate::models::dag::Obligation;
use std::collections::{HashMap, HashSet};

/// A selected obligation with curated context for the solver.
#[derive(Debug, Clone)]
pub struct SelectedObligation {
    pub obligation: Obligation,
    /// Blacklisted approaches for this obligation (technique → failure reason).
    pub blacklisted_approaches: Vec<(String, String)>,
}

/// Tracks consecutive failures per obligation+technique for pivot forcing.
#[derive(Debug, Default)]
pub struct PivotTracker {
    /// Map of obligation_id → Vec<technique_class> (recent failure history).
    failure_runs: HashMap<String, Vec<String>>,
    /// Map of obligation_id → Vec<(technique, reason)> for blacklisted approaches.
    blacklists: HashMap<String, Vec<(String, String)>>,
}

const PIVOT_THRESHOLD: usize = 3;

impl PivotTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a failure on an obligation with a given technique class.
    /// Returns Some((technique, reason)) if a pivot is triggered (3 consecutive same-technique failures).
    pub fn record_failure(
        &mut self,
        obligation_id: &str,
        technique_class: &str,
        failure_reason: &str,
    ) -> Option<(String, String)> {
        let history = self
            .failure_runs
            .entry(obligation_id.to_string())
            .or_default();
        history.push(technique_class.to_string());

        // Check if last PIVOT_THRESHOLD entries are all the same technique
        if history.len() >= PIVOT_THRESHOLD {
            let tail = &history[history.len() - PIVOT_THRESHOLD..];
            if tail.iter().all(|t| t == technique_class) {
                // Pivot triggered — blacklist this technique for this obligation
                let entry = (technique_class.to_string(), failure_reason.to_string());
                self.blacklists
                    .entry(obligation_id.to_string())
                    .or_default()
                    .push(entry.clone());
                // Clear the run so we don't re-trigger immediately
                history.clear();
                return Some(entry);
            }
        }
        None
    }

    /// Record a success — clears the failure run for this obligation.
    pub fn record_success(&mut self, obligation_id: &str) {
        self.failure_runs.remove(obligation_id);
    }

    /// Get blacklisted approaches for an obligation.
    pub fn get_blacklist(&self, obligation_id: &str) -> Vec<(String, String)> {
        self.blacklists
            .get(obligation_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Count of blacklisted techniques for an obligation.
    pub fn blacklisted_count(&self, obligation_id: &str) -> usize {
        self.blacklists
            .get(obligation_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// List of blacklisted technique names for an obligation.
    pub fn blacklisted_techniques(&self, obligation_id: &str) -> Vec<String> {
        self.blacklists
            .get(obligation_id)
            .map(|v| v.iter().map(|(t, _)| t.clone()).collect())
            .unwrap_or_default()
    }
}

/// Select the highest-priority unblocked obligation for the solver.
///
/// Returns None if no obligations exist or all are blocked/closed — in that case
/// the caller falls back to freeform prompt mode.
///
/// Priority ranking:
/// 1. S+ priority (>= 0.95) obligations always first (contradiction resolution)
/// 2. Then by effective priority: `priority * confidence * decay(escalation_level)`
/// 3. Tie-break: fewest attempts first, then oldest
pub fn select_next(
    db: &Database,
    attempt_id: &str,
    _branch_id: i32,
    pivot_tracker: &PivotTracker,
) -> Option<SelectedObligation> {
    // Use attempt-wide open obligations so active work is not lost when branch focus changes.
    let mut obligations = db.get_open_obligations(attempt_id).unwrap_or_default();

    if obligations.is_empty() {
        return None;
    }

    // Build set of open obligation IDs for dependency checking (owned to avoid borrow conflict)
    let open_ids: HashSet<String> = obligations.iter().map(|o| o.id.clone()).collect();

    // Filter out obligations whose dependencies are not yet resolved
    obligations.retain(|ob| {
        if let Some(ref deps_json) = ob.depends_on {
            if let Ok(dep_ids) = serde_json::from_str::<Vec<String>>(deps_json) {
                // Blocked if ANY dependency is still open
                !dep_ids.iter().any(|dep_id| open_ids.contains(dep_id))
            } else {
                true // malformed JSON — treat as unblocked
            }
        } else {
            true // no dependencies — always unblocked
        }
    });

    if obligations.is_empty() {
        return None;
    }

    // Sort by effective priority (descending)
    obligations.sort_by(|a, b| {
        let eff_a = effective_priority(a);
        let eff_b = effective_priority(b);
        eff_b
            .partial_cmp(&eff_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Pick the top unblocked obligation
    let selected = obligations.into_iter().next()?;
    let blacklisted = pivot_tracker.get_blacklist(&selected.id);

    Some(SelectedObligation {
        obligation: selected,
        blacklisted_approaches: blacklisted,
    })
}

/// Select up to `max` highest-priority unblocked obligations for a parallel round.
///
/// Each selected obligation is distinct — already-selected IDs are excluded from
/// subsequent picks so parallel tasks work on different obligations. Returns
/// 0..=max items; returns empty when all obligations are blocked or closed.
pub fn select_multiple(
    db: &Database,
    attempt_id: &str,
    _branch_id: i32,
    pivot_tracker: &PivotTracker,
    max: usize,
) -> Vec<SelectedObligation> {
    let mut results = Vec::with_capacity(max);
    let mut excluded: HashSet<String> = HashSet::new();

    for _ in 0..max {
        // Use attempt-wide open obligations excluding already-selected ones this round.
        let mut obligations = db.get_open_obligations(attempt_id).unwrap_or_default();

        if obligations.is_empty() {
            break;
        }

        // Build open_ids for dependency checking
        let open_ids: HashSet<String> = obligations.iter().map(|o| o.id.clone()).collect();

        // Filter: remove dependencies not yet resolved and already-selected this round
        obligations.retain(|ob| {
            if excluded.contains(&ob.id) {
                return false;
            }
            if let Some(ref deps_json) = ob.depends_on {
                if let Ok(dep_ids) = serde_json::from_str::<Vec<String>>(deps_json) {
                    !dep_ids.iter().any(|dep_id| open_ids.contains(dep_id))
                } else {
                    true
                }
            } else {
                true
            }
        });

        if obligations.is_empty() {
            break;
        }

        // Sort by effective priority
        obligations.sort_by(|a, b| {
            effective_priority(b)
                .partial_cmp(&effective_priority(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if let Some(selected) = obligations.into_iter().next() {
            let blacklisted = pivot_tracker.get_blacklist(&selected.id);
            excluded.insert(selected.id.clone());
            results.push(SelectedObligation {
                obligation: selected,
                blacklisted_approaches: blacklisted,
            });
        } else {
            break;
        }
    }

    results
}

/// Compute effective priority for queue ordering.
/// Decays with escalation level so frequently-escalated obligations sink.
fn effective_priority(ob: &Obligation) -> f64 {
    let decay = 1.0 / (1.0 + ob.escalation_level as f64 * 0.15);
    ob.priority * ob.confidence * decay
}
