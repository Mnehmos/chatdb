use chatdb::db::{Database, StepRecord};

fn setup_db_with_obligation() -> (Database, String, String, String, String) {
    let db = Database::new_in_memory().expect("in-memory db");
    let problem = db
        .create_problem("overnight proof run", "number_theory", "test")
        .expect("create problem");
    let attempt_id = db
        .create_attempt(&problem.id, &["test-model".to_string()])
        .expect("create attempt");
    let node_id = db
        .create_node(
            &attempt_id,
            0,
            "claim",
            None,
            "root node",
            None,
            None,
            None,
            "verified",
            None,
            None,
            None,
            None,
            None,
            Some(1),
            0,
        )
        .expect("create node");
    let obligation_id = db
        .create_obligation(
            &attempt_id,
            0,
            &node_id,
            "classify bonza functions",
            "CLASSIFY",
            0.9,
            0.8,
            Some(1),
            Some(20),
        )
        .expect("create obligation");
    (db, problem.id, attempt_id, node_id, obligation_id)
}

fn sample_step_record<'a>(
    attempt_id: &'a str,
    obligation_id: Option<&'a str>,
    verified: bool,
) -> StepRecord<'a> {
    StepRecord {
        attempt_id,
        parent_step_id: None,
        step_number: 1,
        model: "test-model",
        context_refs: None,
        goal_state: "prove something",
        context_provided: None,
        proposal_type: "claim",
        proposal_natural: "f(1) = 1",
        proposal_formal: Some("f(1) = 1"),
        proposal_reasoning: Some("by inspection"),
        sympy_result: Some("ok"),
        sympy_passed: Some(verified),
        pint_result: None,
        pint_passed: None,
        lean_result: None,
        lean_passed: None,
        verified,
        rejection_reason: (!verified).then_some("validator rejected"),
        model_tokens_in: Some(123),
        model_tokens_out: Some(45),
        wall_time_ms: Some(17),
        challenge_model: None,
        challenge_flaw_found: None,
        challenge_attack: None,
        challenge_confidence: None,
        challenge_fatal: None,
        obligation_id,
        solver_round_id: None,
        solver_worker_id: None,
        solver_dispatch_mode: None,
        stale_sibling: false,
    }
}

#[test]
fn record_step_updates_attempt_token_totals_and_obligation_steps() {
    let (db, _problem_id, attempt_id, _node_id, obligation_id) = setup_db_with_obligation();

    let step_id = db
        .record_step(&sample_step_record(
            &attempt_id,
            Some(&obligation_id),
            false,
        ))
        .expect("record step");
    assert!(!step_id.is_empty());

    let conn = db.conn();
    let (step_count, tokens_in, tokens_out): (i32, i32, i32) = conn
        .query_row(
            "SELECT step_count, cost_tokens_in, cost_tokens_out FROM attempts WHERE id = ?1",
            rusqlite::params![attempt_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("query attempt");
    let obligation_steps: i32 = conn
        .query_row(
            "SELECT steps_spent FROM obligations WHERE id = ?1",
            rusqlite::params![obligation_id],
            |row| row.get(0),
        )
        .expect("query obligation");

    assert_eq!(step_count, 1);
    assert_eq!(tokens_in, 123);
    assert_eq!(tokens_out, 45);
    assert_eq!(obligation_steps, 1);
}

#[test]
fn backfilling_tool_runs_updates_step_tool_call_count() {
    let (db, _problem_id, attempt_id, _node_id, obligation_id) = setup_db_with_obligation();
    let step_id = db
        .record_step(&sample_step_record(
            &attempt_id,
            Some(&obligation_id),
            false,
        ))
        .expect("record step");

    let run_1 = db
        .create_tool_run(
            &attempt_id,
            Some(0),
            None,
            Some(1),
            Some(&obligation_id),
            None,
            Some("session-1"),
            "solver",
            "claim_check",
            "claim_check",
            "sidecar",
            None,
            r#"{"claim":"f(1)=1"}"#,
            None,
            false,
        )
        .expect("create tool run 1");
    let run_2 = db
        .create_tool_run(
            &attempt_id,
            Some(0),
            None,
            Some(1),
            Some(&obligation_id),
            None,
            Some("session-1"),
            "solver",
            "sympy_check",
            "sympy_check",
            "sidecar",
            None,
            r#"{"expr":"1=1"}"#,
            None,
            false,
        )
        .expect("create tool run 2");

    db.backfill_tool_runs_step_id(&[run_1, run_2], &step_id)
        .expect("backfill step id");

    let conn = db.conn();
    let tool_call_count: i32 = conn
        .query_row(
            "SELECT tool_call_count FROM steps WHERE id = ?1",
            rusqlite::params![step_id],
            |row| row.get(0),
        )
        .expect("query tool_call_count");
    drop(conn);

    let tool_runs = db
        .get_tool_runs_for_step(&step_id)
        .expect("query tool runs for step");
    assert_eq!(tool_runs.len(), 2);
    assert_eq!(tool_call_count, 2);
}

#[test]
fn fatal_challenge_demotes_verified_step_and_rebalances_attempt_counters() {
    let (db, _problem_id, attempt_id, _node_id, obligation_id) = setup_db_with_obligation();
    let step_id = db
        .record_step(&sample_step_record(&attempt_id, Some(&obligation_id), true))
        .expect("record verified step");
    db.increment_attempt_counter(&attempt_id, "steps_verified")
        .expect("increment verified counter");

    db.update_step_challenge(
        &step_id,
        "adversary-model",
        true,
        "formal claim is reversed",
        0.99,
        true,
        Some("adversarial veto"),
    )
    .expect("update step challenge");

    let conn = db.conn();
    let (verified, rejection_reason): (i32, Option<String>) = conn
        .query_row(
            "SELECT verified, rejection_reason FROM steps WHERE id = ?1",
            rusqlite::params![step_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("query step");
    let (steps_verified, steps_rejected): (i32, i32) = conn
        .query_row(
            "SELECT steps_verified, steps_rejected FROM attempts WHERE id = ?1",
            rusqlite::params![attempt_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("query attempt counters");

    assert_eq!(verified, 0);
    assert!(rejection_reason.is_some());
    assert_eq!(steps_verified, 0);
    assert_eq!(steps_rejected, 1);
    drop(conn);
    assert!(db
        .get_verified_chain(&attempt_id)
        .expect("verified chain")
        .is_empty());
}
