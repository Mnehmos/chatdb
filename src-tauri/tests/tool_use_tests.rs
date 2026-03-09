/// Integration tests for the V14 Tool-Use Redesign schema and DB operations.
/// Tests cover: tool_runs, tool_artifacts, scout_sessions, obligation_resolutions,
/// resolution_corpus, corpus_usages, tool_policy_violations, and the extended
/// obligation scout metadata fields.

// ---- Schema migration ----

#[test]
fn test_v14_tables_created() {
    let db = chatdb::db::Database::new_in_memory().expect("in-memory DB");
    let conn = db.conn();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    for expected in &[
        "tool_runs",
        "tool_artifacts",
        "scout_sessions",
        "obligation_resolutions",
        "resolution_corpus",
        "corpus_usages",
        "tool_policy_violations",
    ] {
        assert!(
            tables.contains(&expected.to_string()),
            "missing table: {}",
            expected
        );
    }
}

#[test]
fn test_v14_obligation_columns_exist() {
    let db = chatdb::db::Database::new_in_memory().expect("in-memory DB");
    let conn = db.conn();
    // PRAGMA table_info returns all columns for the table
    let cols: Vec<String> = conn
        .prepare("PRAGMA table_info(obligations)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    for expected in &[
        "signature_json",
        "embedding_json",
        "scout_status",
        "last_scout_session_id",
        "last_scout_confidence",
        "resolved_externally",
        "resolved_by_corpus_id",
        "external_reference",
        "scout_last_checked_at",
    ] {
        assert!(
            cols.contains(&expected.to_string()),
            "missing obligation column: {}",
            expected
        );
    }
}

#[test]
fn test_v14_steps_columns_exist() {
    let db = chatdb::db::Database::new_in_memory().expect("in-memory DB");
    let conn = db.conn();
    let cols: Vec<String> = conn
        .prepare("PRAGMA table_info(steps)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    for expected in &[
        "solver_round_id",
        "tool_call_count",
        "sympy_preflight_used",
        "tool_policy_status",
    ] {
        assert!(
            cols.contains(&expected.to_string()),
            "missing steps column: {}",
            expected
        );
    }
}

// ---- Helper: create prerequisite rows ----

fn setup_db() -> (chatdb::db::Database, String, String, String) {
    let db = chatdb::db::Database::new_in_memory().expect("in-memory DB");
    let problem = db
        .create_problem("test problem", "algebra", "test")
        .expect("create problem");
    let attempt_id = db
        .create_attempt(&problem.id, &["test-model".to_string()])
        .expect("create attempt");
    // Create a proof node so we can create obligations
    let node_id = db
        .create_node(
            &attempt_id,
            0,
            "step",
            None,
            "content",
            None,
            None,
            None,
            "proposed",
            None,
            None,
            None,
            None,
            None,
            None,
            1,
        )
        .expect("create node");
    (db, problem.id, attempt_id, node_id)
}

// ---- tool_runs CRUD ----

#[test]
fn test_tool_run_create_and_complete() {
    let (db, _pid, attempt_id, _nid) = setup_db();

    let run_id = db
        .create_tool_run(
            &attempt_id,
            Some(0),
            None,
            Some(1),
            None,
            None,
            None,
            "solver",
            "step_tool_use",
            "sympy_check",
            "sympy",
            Some(1),
            r#"{"expr":"x**2"}"#,
            None,
            false,
        )
        .expect("create_tool_run");

    db.complete_tool_run(
        &run_id,
        "succeeded",
        Some(true),
        Some(150),
        None,
        Some(r#"{"valid":true}"#),
        Some("SymPy check passed"),
    )
    .expect("complete_tool_run");

    let runs = db
        .get_tool_runs_for_step("nonexistent")
        .expect("query empty");
    assert_eq!(runs.len(), 0);

    // Query by obligation — run has no obligation_id so nothing returned
    let runs = db
        .get_tool_runs_for_obligation("nonexistent")
        .expect("query empty");
    assert_eq!(runs.len(), 0);
}

#[test]
fn test_tool_run_query_by_session() {
    let (db, _pid, attempt_id, _nid) = setup_db();
    let session_id = "test-session-001";

    let _r1 = db
        .create_tool_run(
            &attempt_id,
            None,
            None,
            None,
            None,
            None,
            Some(session_id),
            "scout",
            "obligation_preflight",
            "loogle",
            "loogle",
            Some(2),
            r#"{"query":"prime"}"#,
            None,
            false,
        )
        .expect("create run 1");

    let _r2 = db
        .create_tool_run(
            &attempt_id,
            None,
            None,
            None,
            None,
            None,
            Some(session_id),
            "scout",
            "obligation_preflight",
            "oeis",
            "oeis",
            Some(3),
            r#"{"query":"prime sequence"}"#,
            None,
            false,
        )
        .expect("create run 2");

    let runs = db
        .get_tool_runs_for_session(session_id)
        .expect("query by session");
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].tool_name, "loogle");
    assert_eq!(runs[1].tool_name, "oeis");
}

// ---- tool_artifacts CRUD ----

#[test]
fn test_tool_artifact_roundtrip() {
    let (db, _pid, attempt_id, _nid) = setup_db();

    let run_id = db
        .create_tool_run(
            &attempt_id,
            None,
            None,
            None,
            None,
            None,
            None,
            "scout",
            "problem_preflight",
            "arxiv",
            "arxiv",
            Some(5),
            r#"{"query":"fibonacci"}"#,
            None,
            false,
        )
        .expect("create run");

    let a1 = db
        .insert_tool_artifact(
            &run_id,
            &attempt_id,
            None,
            0,
            "paper",
            Some("arxiv:2301.12345"),
            Some("Fibonacci identities"),
            None,
            None,
            Some("https://arxiv.org/abs/2301.12345"),
            Some(0.85),
            r#"{"title":"Fibonacci"}"#,
        )
        .expect("insert artifact 1");

    let a2 = db
        .insert_tool_artifact(
            &run_id,
            &attempt_id,
            None,
            1,
            "theorem",
            None,
            Some("Binet's formula"),
            Some("F_n = (phi^n - psi^n) / sqrt(5)"),
            Some("closed_form"),
            None,
            Some(0.92),
            r#"{"type":"theorem"}"#,
        )
        .expect("insert artifact 2");

    let artifacts = db
        .get_artifacts_for_tool_run(&run_id)
        .expect("query artifacts");
    assert_eq!(artifacts.len(), 2);
    assert_eq!(artifacts[0].id, a1);
    assert_eq!(artifacts[0].artifact_index, 0);
    assert_eq!(artifacts[1].id, a2);
    assert_eq!(artifacts[1].artifact_kind, "theorem");
    assert!((artifacts[1].relevance_score.unwrap() - 0.92).abs() < 0.001);
}

// ---- scout_sessions CRUD ----

#[test]
fn test_scout_session_lifecycle() {
    let (db, pid, attempt_id, node_id) = setup_db();

    let ob_id = db
        .create_obligation(
            &attempt_id,
            0,
            &node_id,
            "Prove prime",
            "proof",
            0.8,
            0.7,
            Some(1),
            None,
        )
        .expect("create obligation");

    let session_id = db
        .create_scout_session(
            &attempt_id,
            Some(0),
            &pid,
            Some(&ob_id),
            "obligation_preflight",
            "new_obligation",
            r#"["self","corpus","loogle"]"#,
        )
        .expect("create scout session");

    let session = db.get_scout_session(&session_id).expect("get session");
    assert_eq!(session.status, "running");
    assert_eq!(session.scope, "obligation_preflight");
    assert_eq!(session.obligation_id.as_deref(), Some(ob_id.as_str()));

    db.complete_scout_session(
        &session_id,
        "resolved",
        Some("resolved"),
        None,
        None,
        Some(0.95),
    )
    .expect("complete session");

    let session = db
        .get_scout_session(&session_id)
        .expect("get completed session");
    assert_eq!(session.status, "resolved");
    assert!((session.confidence.unwrap() - 0.95).abs() < 0.001);
    assert!(session.completed_at.is_some());

    let sessions = db
        .get_scout_sessions_for_obligation(&ob_id)
        .expect("query by obligation");
    assert_eq!(sessions.len(), 1);
}

// ---- obligation_resolutions CRUD ----

#[test]
fn test_obligation_resolution_roundtrip() {
    let (db, pid, attempt_id, node_id) = setup_db();

    let ob_id = db
        .create_obligation(
            &attempt_id,
            0,
            &node_id,
            "Prove Binet's formula",
            "lemma",
            0.9,
            0.6,
            None,
            None,
        )
        .expect("create obligation");

    let session_id = db
        .create_scout_session(
            &attempt_id,
            None,
            &pid,
            Some(&ob_id),
            "obligation_preflight",
            "new_obligation",
            "[]",
        )
        .expect("create session");

    let res_id = db
        .create_obligation_resolution(
            &attempt_id,
            &ob_id,
            &session_id,
            "resolved",
            3,
            "loogle",
            "Nat.fib_formula",
            Some("https://loogle.lean-lang.org"),
            "lean4",
            "Direct match: Nat.fib_formula proves Binet's formula",
            Some(r#"{"lean_name":"Nat.fib_formula"}"#),
            true,
            0.98,
            None,
        )
        .expect("create resolution");

    let resolutions = db
        .get_resolutions_for_obligation(&ob_id)
        .expect("query resolutions");
    assert_eq!(resolutions.len(), 1);
    assert_eq!(resolutions[0].id, res_id);
    assert_eq!(resolutions[0].status, "resolved");
    assert!(resolutions[0].mechanical_verification);
    assert!((resolutions[0].confidence - 0.98).abs() < 0.001);
}

// ---- resolution_corpus CRUD ----

#[test]
fn test_corpus_register_and_find_by_hash() {
    let (db, _pid, _attempt_id, _nid) = setup_db();

    let hash = "sha256:abc123def456";
    let entry_id = db
        .register_corpus_entry(
            "For all n >= 1, F_n = (phi^n - psi^n)/sqrt(5)",
            hash,
            "lemma",
            Some("number_theory"),
            r#"{"obligation_type":"lemma","domain":"number_theory"}"#,
            None,
            3,
            "loogle",
            "Nat.fib_formula",
            Some("https://loogle.lean-lang.org"),
            "lean4",
            "Binet's formula",
            r#"{"lean_name":"Nat.fib_formula"}"#,
            true,
            Some("lean4_check"),
            Some("Nat.fib_formula"),
            None,
        )
        .expect("register corpus entry");

    // Find by hash
    let found = db.find_corpus_by_hash(hash).expect("find by hash");
    assert!(found.is_some());
    let entry = found.unwrap();
    assert_eq!(entry.id, entry_id);
    assert_eq!(entry.canonical_hash, hash);
    assert!(entry.mechanical_verification);
    assert_eq!(entry.reuse_count, 0);

    // Increment reuse
    db.increment_corpus_reuse(&entry_id)
        .expect("increment reuse");
    let found = db.find_corpus_by_hash(hash).expect("find after reuse");
    assert_eq!(found.unwrap().reuse_count, 1);

    // Miss on unknown hash
    let miss = db
        .find_corpus_by_hash("sha256:nonexistent")
        .expect("find miss");
    assert!(miss.is_none());
}

#[test]
fn test_corpus_top_entries() {
    let (db, _pid, _attempt_id, _nid) = setup_db();

    for i in 0..3 {
        let hash = format!("hash-{}", i);
        let id = db
            .register_corpus_entry(
                &format!("Statement {}", i),
                &hash,
                "proof",
                None,
                "{}",
                None,
                1,
                "self",
                "self",
                None,
                "natural",
                "mapping",
                "{}",
                false,
                None,
                None,
                None,
            )
            .expect("register");
        for _ in 0..i {
            db.increment_corpus_reuse(&id).expect("increment");
        }
    }

    let top = db.get_top_corpus_entries(10).expect("get top");
    assert_eq!(top.len(), 3);
    // Should be sorted by reuse_count DESC
    assert!(top[0].reuse_count >= top[1].reuse_count);
    assert!(top[1].reuse_count >= top[2].reuse_count);
}

// ---- corpus_usages CRUD ----

#[test]
fn test_corpus_usage_roundtrip() {
    let (db, pid, attempt_id, node_id) = setup_db();

    let ob_id = db
        .create_obligation(
            &attempt_id,
            0,
            &node_id,
            "test ob",
            "proof",
            0.5,
            0.5,
            None,
            None,
        )
        .expect("create obligation");

    let session_id = db
        .create_scout_session(
            &attempt_id,
            None,
            &pid,
            Some(&ob_id),
            "obligation_preflight",
            "new_obligation",
            "[]",
        )
        .expect("create session");

    let corpus_id = db
        .register_corpus_entry(
            "test statement",
            "hash-test",
            "proof",
            None,
            "{}",
            None,
            1,
            "self",
            "self",
            None,
            "natural",
            "mapping",
            "{}",
            false,
            None,
            None,
            None,
        )
        .expect("register");

    let usage_id = db
        .record_corpus_usage(&corpus_id, &attempt_id, &pid, &ob_id, &session_id, None)
        .expect("record usage");

    let usages = db
        .get_usages_for_corpus_entry(&corpus_id)
        .expect("query usages");
    assert_eq!(usages.len(), 1);
    assert_eq!(usages[0].id, usage_id);
    assert_eq!(usages[0].obligation_id, ob_id);
}

// ---- tool_policy_violations CRUD ----

#[test]
fn test_tool_policy_violation_roundtrip() {
    let (db, _pid, attempt_id, _nid) = setup_db();

    let v_id = db
        .record_tool_policy_violation(
            &attempt_id,
            Some(0),
            Some("step-123"),
            Some(5),
            None,
            "solver",
            "sympy_preflight_required",
            "sympy_check",
            Some("[]"),
            "retry",
            Some("Equality-bearing step missing SymPy preflight"),
        )
        .expect("record violation");

    let violations = db
        .get_violations_for_attempt(&attempt_id)
        .expect("query violations");
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].id, v_id);
    assert_eq!(violations[0].policy_name, "sympy_preflight_required");
    assert_eq!(violations[0].action_taken, "retry");
}

// ---- Obligation scout metadata updates ----

#[test]
fn test_obligation_scout_metadata_update() {
    let (db, _pid, attempt_id, node_id) = setup_db();

    let ob_id = db
        .create_obligation(
            &attempt_id,
            0,
            &node_id,
            "test obligation",
            "proof",
            0.5,
            0.5,
            None,
            None,
        )
        .expect("create obligation");

    // Initially no scout metadata
    let ob = db.get_obligation(&ob_id).expect("get obligation");
    assert!(ob.scout_status.is_none());
    assert!(!ob.resolved_externally);

    // Update scout status
    db.update_obligation_scout_status(&ob_id, "novel", "session-001", 0.3)
        .expect("update scout status");
    let ob = db.get_obligation(&ob_id).expect("get after scout");
    assert_eq!(ob.scout_status.as_deref(), Some("novel"));
    assert_eq!(ob.last_scout_session_id.as_deref(), Some("session-001"));
    assert!((ob.last_scout_confidence.unwrap() - 0.3).abs() < 0.001);
    assert!(ob.scout_last_checked_at.is_some());

    // Mark externally resolved
    db.mark_obligation_externally_resolved(&ob_id, Some("corpus-001"), "Nat.fib_formula")
        .expect("mark externally resolved");
    let ob = db
        .get_obligation(&ob_id)
        .expect("get after external resolve");
    assert!(ob.resolved_externally);
    assert_eq!(ob.resolved_by_corpus_id.as_deref(), Some("corpus-001"));
    assert_eq!(ob.external_reference.as_deref(), Some("Nat.fib_formula"));
}

// ---- Delete cascade ----

#[test]
fn test_delete_attempt_cascades_v14_tables() {
    let (db, pid, attempt_id, node_id) = setup_db();

    // Create data in all V14 tables
    let ob_id = db
        .create_obligation(
            &attempt_id,
            0,
            &node_id,
            "test ob",
            "proof",
            0.5,
            0.5,
            None,
            None,
        )
        .expect("create obligation");

    let run_id = db
        .create_tool_run(
            &attempt_id,
            None,
            None,
            None,
            Some(&ob_id),
            None,
            None,
            "scout",
            "preflight",
            "loogle",
            "loogle",
            Some(2),
            "{}",
            None,
            false,
        )
        .expect("create tool run");

    let _artifact_id = db
        .insert_tool_artifact(
            &run_id,
            &attempt_id,
            Some(&ob_id),
            0,
            "theorem",
            None,
            None,
            None,
            None,
            None,
            None,
            "{}",
        )
        .expect("insert artifact");

    let session_id = db
        .create_scout_session(
            &attempt_id,
            None,
            &pid,
            Some(&ob_id),
            "obligation_preflight",
            "new_obligation",
            "[]",
        )
        .expect("create session");

    let _res_id = db
        .create_obligation_resolution(
            &attempt_id,
            &ob_id,
            &session_id,
            "resolved",
            2,
            "loogle",
            "ref",
            None,
            "lean4",
            "mapping",
            None,
            false,
            0.8,
            None,
        )
        .expect("create resolution");

    let corpus_id = db
        .register_corpus_entry(
            "stmt",
            "hash-cascade-test",
            "proof",
            None,
            "{}",
            None,
            1,
            "self",
            "self",
            None,
            "natural",
            "mapping",
            "{}",
            false,
            None,
            None,
            None,
        )
        .expect("register corpus");

    let _usage_id = db
        .record_corpus_usage(&corpus_id, &attempt_id, &pid, &ob_id, &session_id, None)
        .expect("record usage");

    let _violation_id = db
        .record_tool_policy_violation(
            &attempt_id,
            None,
            None,
            None,
            None,
            "solver",
            "test_policy",
            "sympy_check",
            None,
            "retry",
            None,
        )
        .expect("record violation");

    // Delete attempt — should cascade all V14 tables
    db.delete_attempt(&attempt_id).expect("delete attempt");

    // Verify all V14 data is gone
    let conn = db.conn();
    for table in &[
        "tool_runs",
        "tool_artifacts",
        "scout_sessions",
        "obligation_resolutions",
        "corpus_usages",
        "tool_policy_violations",
    ] {
        let count: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM {} WHERE attempt_id = ?1", table),
                rusqlite::params![&attempt_id],
                |row| row.get(0),
            )
            .expect(&format!("count {}", table));
        assert_eq!(count, 0, "{} should be empty after delete_attempt", table);
    }

    // Corpus entries are NOT deleted by attempt cascade (they're permanent)
    let corpus_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM resolution_corpus WHERE canonical_hash = 'hash-cascade-test'",
            [],
            |row| row.get(0),
        )
        .expect("count corpus");
    assert_eq!(
        corpus_count, 1,
        "resolution_corpus entries should survive attempt deletion"
    );
}
