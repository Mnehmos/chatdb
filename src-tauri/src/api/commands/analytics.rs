use crate::api::sidecar::{
    FormalizeProofObligation, FormalizeProofRequest, FormalizeProofStep, SidecarClient,
};
use crate::models::agents::TrainingDataStats;
use crate::models::dag::Obligation;
use crate::models::proof::{
    AfterActionReport, LeanFormalizationResult, Problem, Step, TrainingRow,
};
use crate::AppState;
use std::sync::Arc;

#[tauri::command]
pub fn get_training_data_stats(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<TrainingDataStats, String> {
    let (total, verified, rejected, contrastive_pairs) = state
        .db
        .get_training_data_stats()
        .map_err(|e| e.to_string())?;
    Ok(TrainingDataStats {
        total_steps: total,
        verified_steps: verified,
        rejected_steps: rejected,
        contrastive_pairs,
        orchestrator_decisions: 0,
        council_sessions: 0,
        council_findings: 0,
        critic_evaluations: 0,
        scout_queries: 0,
        librarian_actions: 0,
    })
}

#[tauri::command]
pub fn list_all_steps(
    state: tauri::State<'_, Arc<AppState>>,
    limit: Option<u32>,
) -> Result<Vec<TrainingRow>, String> {
    state
        .db
        .list_all_steps(limit.unwrap_or(200))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_after_action_report(
    state: tauri::State<'_, Arc<AppState>>,
    problem_id: String,
) -> Result<AfterActionReport, String> {
    state
        .db
        .get_after_action_report(&problem_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn formalize_proof(
    state: tauri::State<'_, Arc<AppState>>,
    problem_id: String,
) -> Result<LeanFormalizationResult, String> {
    let report = state
        .db
        .get_after_action_report(&problem_id)
        .map_err(|e| e.to_string())?;
    let problem = state
        .db
        .get_problem(&problem_id)
        .map_err(|e| e.to_string())?;
    let steps = state
        .db
        .get_attempt_steps(&report.attempt_id)
        .map_err(|e| e.to_string())?;
    let obligations = state
        .db
        .get_all_obligations(&report.attempt_id)
        .map_err(|e| e.to_string())?;

    let request = build_formalize_proof_request(&problem, &report, &steps, &obligations)?;
    SidecarClient::new()
        .formalize_proof(&request)
        .await
        .map_err(|e| e.to_string())
}

fn build_formalize_proof_request(
    problem: &Problem,
    report: &AfterActionReport,
    steps: &[Step],
    obligations: &[Obligation],
) -> Result<FormalizeProofRequest, String> {
    if !report.proof_complete {
        return Err(
            "Proof is not complete; Lean formalization is only available for completed attempts."
                .into(),
        );
    }

    let mut verified_chain: Vec<FormalizeProofStep> = steps
        .iter()
        .filter(|step| step.attempt_id == report.attempt_id && step.verified && !step.stale_sibling)
        .map(|step| FormalizeProofStep {
            step_number: step.step_number,
            proposal_type: step.proposal_type.clone(),
            natural: step.proposal_natural.clone(),
            formal: step
                .proposal_formal
                .clone()
                .filter(|formal| !formal.trim().is_empty()),
            model: step.model.clone(),
            obligation_id: step.obligation_id.clone(),
            obligation_desc: step.obligation_desc.clone(),
            obligation_type: step.obligation_type.clone(),
        })
        .collect();
    verified_chain.sort_by_key(|step| step.step_number);

    if verified_chain.is_empty() {
        return Err("No verified steps available for Lean formalization.".into());
    }

    let mut obligation_rows: Vec<FormalizeProofObligation> = obligations
        .iter()
        .filter(|obligation| obligation.attempt_id == report.attempt_id)
        .map(|obligation| FormalizeProofObligation {
            id: obligation.id.clone(),
            description: obligation.description.clone(),
            obligation_type: obligation.obligation_type.clone(),
            status: obligation.status.clone(),
        })
        .collect();
    obligation_rows.sort_by(|left, right| right.id.cmp(&left.id));
    obligation_rows.sort_by(|left, right| left.status.cmp(&right.status));

    Ok(FormalizeProofRequest {
        problem_id: problem.id.clone(),
        problem_statement: problem.statement.clone(),
        problem_domain: (!problem.domain.trim().is_empty()).then(|| problem.domain.clone()),
        problem_formal_statement: problem.formal_statement.clone(),
        attempt_id: report.attempt_id.clone(),
        final_answer: report.final_answer.clone(),
        verified_chain,
        obligations: obligation_rows,
    })
}

#[cfg(test)]
mod tests {
    use super::build_formalize_proof_request;

    #[test]
    fn build_formalize_request_uses_verified_steps_from_latest_attempt() {
        let problem = crate::models::proof::Problem {
            id: "problem-1".into(),
            statement: "Prove x^2 >= 0".into(),
            formal_statement: Some("x**2 >= 0".into()),
            domain: "algebra".into(),
            source: "test".into(),
            status: "open".into(),
            created_at: "2026-03-06T00:00:00Z".into(),
            solved_at: None,
            total_attempts: 1,
            total_steps: 3,
            known_answer: None,
            title: None,
            difficulty: None,
            metadata: None,
        };
        let report = crate::models::proof::AfterActionReport {
            problem_id: "problem-1".into(),
            problem_statement: "Prove x^2 >= 0".into(),
            problem_domain: "algebra".into(),
            attempt_id: "attempt-2".into(),
            total_steps: 3,
            verified_steps: 2,
            rejected_steps: 1,
            accuracy_pct: 66.0,
            total_tokens_in: 10,
            total_tokens_out: 5,
            total_wall_ms: 25,
            models_used: vec!["solver-a".into()],
            verified_chain: vec![],
            failure_modes: vec![],
            started_at: "2026-03-06T00:00:00Z".into(),
            proof_complete: true,
            open_obligations: 0,
            final_answer: Some("Therefore x^2 >= 0.".into()),
        };
        let stale_older = crate::models::proof::Step {
            id: "step-older".into(),
            attempt_id: "attempt-1".into(),
            parent_step_id: None,
            step_number: 1,
            model: "solver-old".into(),
            goal_state: "old".into(),
            proposal_type: "lemma".into(),
            proposal_natural: "Old branch".into(),
            proposal_formal: Some("x = x".into()),
            proposal_reasoning: None,
            verified: true,
            rejection_reason: None,
            sympy_passed: Some(true),
            pint_passed: None,
            lean_passed: None,
            challenge_model: None,
            challenge_flaw_found: None,
            challenge_attack: None,
            challenge_confidence: None,
            challenge_fatal: None,
            obligation_id: Some("ob-old".into()),
            obligation_desc: Some("old obligation".into()),
            obligation_type: Some("BOUND".into()),
            solver_round_id: None,
            solver_worker_id: None,
            solver_dispatch_mode: None,
            stale_sibling: false,
            created_at: "2026-03-05T00:00:00Z".into(),
        };
        let verified_latest = crate::models::proof::Step {
            id: "step-1".into(),
            attempt_id: "attempt-2".into(),
            parent_step_id: None,
            step_number: 1,
            model: "solver-a".into(),
            goal_state: "prove nonnegativity".into(),
            proposal_type: "lemma".into(),
            proposal_natural: "Squares are nonnegative.".into(),
            proposal_formal: Some("x**2 >= 0".into()),
            proposal_reasoning: None,
            verified: true,
            rejection_reason: None,
            sympy_passed: Some(true),
            pint_passed: None,
            lean_passed: None,
            challenge_model: None,
            challenge_flaw_found: None,
            challenge_attack: None,
            challenge_confidence: None,
            challenge_fatal: None,
            obligation_id: Some("ob-1".into()),
            obligation_desc: Some("show the square is nonnegative".into()),
            obligation_type: Some("BOUND".into()),
            solver_round_id: None,
            solver_worker_id: None,
            solver_dispatch_mode: None,
            stale_sibling: false,
            created_at: "2026-03-06T00:00:00Z".into(),
        };
        let stale_latest = crate::models::proof::Step {
            id: "step-2".into(),
            attempt_id: "attempt-2".into(),
            parent_step_id: None,
            step_number: 2,
            model: "solver-b".into(),
            goal_state: "prove nonnegativity".into(),
            proposal_type: "lemma".into(),
            proposal_natural: "Parallel duplicate.".into(),
            proposal_formal: Some("x**2 >= 0".into()),
            proposal_reasoning: None,
            verified: true,
            rejection_reason: None,
            sympy_passed: Some(true),
            pint_passed: None,
            lean_passed: None,
            challenge_model: None,
            challenge_flaw_found: None,
            challenge_attack: None,
            challenge_confidence: None,
            challenge_fatal: None,
            obligation_id: Some("ob-1".into()),
            obligation_desc: Some("show the square is nonnegative".into()),
            obligation_type: Some("BOUND".into()),
            solver_round_id: None,
            solver_worker_id: None,
            solver_dispatch_mode: None,
            stale_sibling: true,
            created_at: "2026-03-06T00:01:00Z".into(),
        };
        let obligations = vec![
            crate::models::dag::Obligation {
                id: "ob-1".into(),
                attempt_id: "attempt-2".into(),
                branch_id: 0,
                parent_node_id: "node-1".into(),
                description: "show the square is nonnegative".into(),
                obligation_type: "BOUND".into(),
                priority: 0.9,
                confidence: 0.8,
                source_layer: Some(1),
                status: "closed_proved".into(),
                assigned_model: None,
                closure_node_id: None,
                closure_type: None,
                escalation_level: 0,
                steps_spent: 1,
                max_steps: 20,
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
                created_at: "2026-03-06T00:00:00Z".into(),
                closed_at: Some("2026-03-06T00:05:00Z".into()),
            },
            crate::models::dag::Obligation {
                id: "ob-2".into(),
                attempt_id: "attempt-2".into(),
                branch_id: 0,
                parent_node_id: "node-1".into(),
                description: "unused open obligation".into(),
                obligation_type: "CASE_CHECK".into(),
                priority: 0.3,
                confidence: 0.4,
                source_layer: Some(1),
                status: "open".into(),
                assigned_model: None,
                closure_node_id: None,
                closure_type: None,
                escalation_level: 0,
                steps_spent: 0,
                max_steps: 20,
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
                created_at: "2026-03-06T00:00:00Z".into(),
                closed_at: None,
            },
        ];

        let request = build_formalize_proof_request(
            &problem,
            &report,
            &[stale_older, verified_latest, stale_latest],
            &obligations,
        )
        .expect("build request");

        assert_eq!(request.problem_id, "problem-1");
        assert_eq!(
            request.problem_formal_statement.as_deref(),
            Some("x**2 >= 0")
        );
        assert_eq!(request.verified_chain.len(), 1);
        assert_eq!(request.verified_chain[0].step_number, 1);
        assert_eq!(
            request.verified_chain[0].obligation_id.as_deref(),
            Some("ob-1")
        );
        assert_eq!(request.obligations.len(), 2);
        assert_eq!(request.obligations[0].id, "ob-1");
    }
}
