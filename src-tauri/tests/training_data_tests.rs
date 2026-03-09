use chatdb::db::{Database, StepRecord};

fn setup_db_with_obligation() -> (Database, String, String) {
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
    (db, attempt_id, obligation_id)
}

fn sample_step_record<'a>(attempt_id: &'a str, obligation_id: &'a str) -> StepRecord<'a> {
    StepRecord {
        attempt_id,
        parent_step_id: None,
        step_number: 1,
        model: "test-model",
        context_refs: None,
        goal_state: "prove something",
        context_provided: None,
        proposal_type: "lemma",
        proposal_natural: "f(1) = 1",
        proposal_formal: Some("1 = 1"),
        proposal_reasoning: Some("by inspection"),
        sympy_result: Some("ok"),
        sympy_passed: Some(true),
        pint_result: None,
        pint_passed: None,
        lean_result: None,
        lean_passed: None,
        verified: true,
        rejection_reason: None,
        model_tokens_in: Some(123),
        model_tokens_out: Some(45),
        wall_time_ms: Some(17),
        challenge_model: None,
        challenge_flaw_found: None,
        challenge_attack: None,
        challenge_confidence: None,
        challenge_fatal: None,
        obligation_id: Some(obligation_id),
        solver_round_id: None,
        solver_worker_id: None,
        solver_dispatch_mode: None,
        stale_sibling: false,
    }
}

#[test]
fn attempt_steps_include_obligation_metadata_for_linked_steps() {
    let (db, attempt_id, obligation_id) = setup_db_with_obligation();
    db.record_step(&sample_step_record(&attempt_id, &obligation_id))
        .expect("record step");

    let steps = db
        .get_attempt_steps(&attempt_id)
        .expect("get attempt steps");
    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0].obligation_id.as_deref(),
        Some(obligation_id.as_str())
    );
    assert_eq!(
        steps[0].obligation_desc.as_deref(),
        Some("classify bonza functions")
    );
    assert_eq!(steps[0].obligation_type.as_deref(), Some("CLASSIFY"));
}

#[test]
fn training_rows_expose_obligation_linkage_fields_for_export_surfaces() {
    let (db, attempt_id, obligation_id) = setup_db_with_obligation();
    db.record_step(&sample_step_record(&attempt_id, &obligation_id))
        .expect("record step");

    let rows = db.list_all_steps(10).expect("list all steps");
    assert_eq!(rows.len(), 1);

    let row_json = serde_json::to_value(&rows[0]).expect("serialize training row");
    assert_eq!(row_json["attempt_id"], attempt_id);
    assert_eq!(row_json["obligation_id"], obligation_id);
    assert_eq!(row_json["obligation_desc"], "classify bonza functions");
    assert_eq!(row_json["obligation_type"], "CLASSIFY");
    assert_eq!(row_json["stale_sibling"], false);
}

#[test]
fn duplicate_verified_steps_are_labeled_semantic_redundant_in_training_rows() {
    let (db, attempt_id, obligation_id) = setup_db_with_obligation();
    let first = sample_step_record(&attempt_id, &obligation_id);
    db.record_step(&first).expect("record first step");

    let mut second = sample_step_record(&attempt_id, &obligation_id);
    second.step_number = 2;
    second.proposal_natural = "  F(1) = 1  ";
    db.record_step(&second).expect("record second step");

    let rows = db.list_all_steps(10).expect("list all steps");
    assert_eq!(rows.len(), 2);

    let first_row = rows
        .iter()
        .find(|row| row.step_number == 1)
        .expect("first row present");
    let second_row = rows
        .iter()
        .find(|row| row.step_number == 2)
        .expect("second row present");

    let first_json = serde_json::to_value(first_row).expect("serialize first row");
    let second_json = serde_json::to_value(second_row).expect("serialize second row");

    assert_eq!(first_json["semantic_redundant"], false);
    assert_eq!(second_json["semantic_redundant"], true);
}
