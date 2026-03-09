use std::collections::HashMap;

/// Branch decision from plateau detection.
#[derive(Debug, Clone)]
pub enum BranchDecision {
    /// Stay on current branch — no plateau detected.
    Continue,
    /// Fork from current node — plateau detected, explore alternative.
    Branch { reason: String },
    /// Abandon current branch — too many failures, switch to another.
    Abandon { reason: String },
}

/// Plateau detection thresholds.
const TECHNIQUE_TUNNEL_THRESHOLD: u32 = 5;
const STEPS_WITHOUT_CLOSURE_THRESHOLD: u32 = 10;
const CLOSURE_RATE_THRESHOLD: f64 = 0.3;
const CLOSURE_WINDOW_SIZE: usize = 5;

pub struct Orchestrator {
    failure_counts: HashMap<String, u32>,
    /// Current active branch.
    pub active_branch_id: i32,
    /// Consecutive steps in the same technique family (per branch).
    technique_run_length: u32,
    last_technique_family: Option<String>,
    /// Steps since last obligation closure (per branch).
    steps_since_last_closure: u32,
    /// Sliding window of closure events: true = obligation closed, false = step without closure.
    closure_window: Vec<bool>,
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            failure_counts: HashMap::new(),
            active_branch_id: 0,
            technique_run_length: 0,
            last_technique_family: None,
            steps_since_last_closure: 0,
            closure_window: Vec::new(),
        }
    }

    pub fn record_outcome(&mut self, model: &str, verified: bool) {
        if verified {
            self.failure_counts.insert(model.to_string(), 0);
        } else {
            *self.failure_counts.entry(model.to_string()).or_insert(0) += 1;
        }
    }

    pub fn should_rotate(&self, model: &str, threshold: u32) -> bool {
        self.failure_counts.get(model).copied().unwrap_or(0) >= threshold
    }

    // ---- Plateau Detection ----

    /// Record the technique family of a verified step. Returns true if a
    /// technique tunnel is detected (N+ consecutive steps in same family).
    pub fn record_technique(&mut self, family: &str) -> bool {
        if self.last_technique_family.as_deref() == Some(family) {
            self.technique_run_length += 1;
        } else {
            self.last_technique_family = Some(family.to_string());
            self.technique_run_length = 1;
        }
        self.technique_run_length >= TECHNIQUE_TUNNEL_THRESHOLD
    }

    /// Record whether a verified step closed any obligations.
    pub fn record_closure_event(&mut self, closed: bool) {
        if closed {
            self.steps_since_last_closure = 0;
        } else {
            self.steps_since_last_closure += 1;
        }
        self.closure_window.push(closed);
        if self.closure_window.len() > CLOSURE_WINDOW_SIZE {
            self.closure_window.remove(0);
        }
    }

    /// Get the obligation closure rate over the sliding window.
    pub fn get_closure_rate(&self) -> f64 {
        if self.closure_window.is_empty() {
            return 1.0; // no data = assume healthy
        }
        let closed = self.closure_window.iter().filter(|&&c| c).count();
        closed as f64 / self.closure_window.len() as f64
    }

    /// Core branching decision. Called after audit says `should_branch`.
    /// Combines multiple plateau signals to decide whether to actually branch.
    pub fn should_branch(&self) -> BranchDecision {
        let mut reasons = Vec::new();

        // Signal 1: technique tunnel
        if self.technique_run_length >= TECHNIQUE_TUNNEL_THRESHOLD {
            reasons.push(format!(
                "technique tunnel: {} consecutive steps in '{}'",
                self.technique_run_length,
                self.last_technique_family.as_deref().unwrap_or("unknown")
            ));
        }

        // Signal 2: no obligation closure for too long
        if self.steps_since_last_closure >= STEPS_WITHOUT_CLOSURE_THRESHOLD {
            reasons.push(format!(
                "stalled: {} steps without closing any obligation",
                self.steps_since_last_closure
            ));
        }

        // Signal 3: closure rate below threshold
        let rate = self.get_closure_rate();
        if self.closure_window.len() >= CLOSURE_WINDOW_SIZE && rate < CLOSURE_RATE_THRESHOLD {
            reasons.push(format!(
                "low closure rate: {:.0}% over last {} steps",
                rate * 100.0,
                self.closure_window.len()
            ));
        }

        if reasons.is_empty() {
            BranchDecision::Continue
        } else {
            BranchDecision::Branch {
                reason: reasons.join("; "),
            }
        }
    }

    /// Reset plateau tracking — called when switching to a new branch.
    pub fn reset_plateau_state(&mut self) {
        self.technique_run_length = 0;
        self.last_technique_family = None;
        self.steps_since_last_closure = 0;
        self.closure_window.clear();
    }
}
