//! Context Enricher — pre-computes rich context for all agents from DAG history.
//!
//! Before the proof loop starts, the enricher queries the database for:
//! - Prior attempt summaries (what was tried, what worked, what failed)
//! - Prior review findings (technique gaps, exploration blind spots)
//! - Pattern success/failure statistics
//! - Research briefing (from scout)
//!
//! This context is then injected into all agent prompts — solver, reviewer,
//! critic, auditor — so every agent has awareness of the problem's full history.

use crate::db::Database;

/// Pre-computed context available to all agents during a proof run.
#[derive(Debug, Clone)]
pub struct ProofContext {
    /// Summary of prior attempts on this problem.
    pub prior_attempts_summary: String,
    /// Key findings from prior reviews (technique gaps, blind spots).
    pub prior_review_summary: String,
    /// Research briefing from scout (papers, theorems, sequences).
    pub research_briefing: String,
    /// Pattern success statistics for this problem domain.
    pub pattern_stats: String,
}

impl ProofContext {
    /// Build enriched context from database history and research results.
    pub fn build(
        db: &Database,
        problem_id: &str,
        domain: &str,
        current_attempt_id: &str,
        research_briefing: String,
    ) -> Self {
        let prior_attempts_summary =
            build_prior_attempts_summary(db, problem_id, current_attempt_id);
        let prior_review_summary = build_prior_review_summary(db, problem_id);
        let pattern_stats = build_pattern_stats(db, domain);

        ProofContext {
            prior_attempts_summary,
            prior_review_summary,
            research_briefing,
            pattern_stats,
        }
    }

    /// Format context for injection into a solver prompt.
    /// Returns a compact block that goes after the problem statement.
    pub fn format_for_solver(&self) -> String {
        let mut ctx = String::new();

        if !self.research_briefing.is_empty() {
            ctx.push_str(&self.research_briefing);
            ctx.push_str("\n\n");
        }

        if !self.prior_attempts_summary.is_empty() {
            ctx.push_str(&self.prior_attempts_summary);
            ctx.push_str("\n\n");
        }

        if !self.prior_review_summary.is_empty() {
            ctx.push_str(&self.prior_review_summary);
            ctx.push_str("\n\n");
        }

        ctx
    }

    /// Format context for injection into a reviewer/critic/auditor prompt.
    /// Includes everything the solver gets plus pattern stats.
    pub fn format_for_analyst(&self) -> String {
        let mut ctx = self.format_for_solver();

        if !self.pattern_stats.is_empty() {
            ctx.push_str(&self.pattern_stats);
            ctx.push_str("\n\n");
        }

        ctx
    }
}

/// Build a summary of prior attempts on this problem.
fn build_prior_attempts_summary(
    db: &Database,
    problem_id: &str,
    current_attempt_id: &str,
) -> String {
    let attempts = match db.list_attempts_for_problem(problem_id) {
        Ok(a) => a,
        Err(_) => return String::new(),
    };

    // Filter out current attempt
    let prior: Vec<_> = attempts
        .iter()
        .filter(|a| a.id != current_attempt_id)
        .take(5) // Last 5 prior attempts
        .collect();

    if prior.is_empty() {
        return String::new();
    }

    let mut summary = format!(
        "PRIOR ATTEMPTS ({} previous attempts on this problem):\n",
        prior.len()
    );
    for a in &prior {
        let status = &a.status;
        let steps_v = a.steps_verified;
        let steps_r = a.steps_rejected;
        let strategy = a.strategy.as_deref().unwrap_or("freeform");
        let coverage = a.coverage.unwrap_or(0.0);

        summary.push_str(&format!(
            "  Attempt #{}: {} — {} verified, {} rejected, coverage {:.0}%, strategy: {}\n",
            a.attempt_number.unwrap_or(0),
            status,
            steps_v,
            steps_r,
            coverage * 100.0,
            strategy,
        ));
    }

    // Add learning instruction
    summary.push_str(
        "  Learn from these: avoid repeating failed approaches, build on partial successes.\n",
    );

    summary
}

/// Build a summary of findings from prior reviews.
fn build_prior_review_summary(db: &Database, problem_id: &str) -> String {
    let findings = match db.get_findings_for_problem(problem_id) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };

    if findings.is_empty() {
        return String::new();
    }

    let mut summary = String::from(
        "PRIOR REVIEW FINDINGS (from analysis of previous attempts — address these gaps):\n",
    );

    // Deduplicate findings by summary (same gap found across attempts)
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut count = 0;

    for f in &findings {
        let key = f.summary.chars().take(40).collect::<String>();
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        summary.push_str(&format!(
            "  [{}] {}: {}\n",
            f.finding_type, f.summary, f.detail
        ));
        count += 1;
        if count >= 8 {
            break;
        }
    }

    summary
}

/// Build pattern success/failure statistics for this domain.
fn build_pattern_stats(db: &Database, domain: &str) -> String {
    let patterns = match db.search_patterns(domain) {
        Ok(p) => p,
        Err(_) => return String::new(),
    };

    if patterns.is_empty() {
        return String::new();
    }

    let mut summary =
        String::from("TECHNIQUE PERFORMANCE (from prior proof runs on similar problems):\n");

    for p in patterns.iter().take(8) {
        let total = p.success_count + p.failure_count;
        if total == 0 {
            continue;
        }
        let rate = p.success_count as f64 / total as f64;
        let label = if rate >= 0.7 {
            "reliable"
        } else if rate >= 0.4 {
            "mixed"
        } else {
            "unreliable"
        };
        let technique = p.technique_class.as_deref().unwrap_or(&p.name);
        summary.push_str(&format!(
            "  {} — {}: {}/{} success ({:.0}%) | {}\n",
            technique,
            label,
            p.success_count,
            total,
            rate * 100.0,
            p.description,
        ));
    }

    summary
}
