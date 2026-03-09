/// Red-phase tests for the mid-run Discerner agent.
/// These test the pure functions and DB operations for in-flight failure classification.
/// All tests here reference items that must exist after the Green phase is implemented.
use chatdb::loop_engine::discerner::{
    build_mid_run_prompt, extract_http_status, parse_mid_run_verdict, FailureBuffer, FailureEntry,
};

fn make_entry(
    failure_type: &str,
    category: &str,
    reason: &str,
    http_status: Option<&str>,
) -> FailureEntry {
    FailureEntry {
        step_number: Some(1),
        ts: "2026-02-28T00:00:00.000Z".to_string(),
        failure_type: failure_type.to_string(),
        category: category.to_string(),
        reason: reason.to_string(),
        http_status: http_status.map(String::from),
        model: "anthropic/claude-sonnet-4-6".to_string(),
        proposal_natural: None,
    }
}

// ---- FailureBuffer behavior ----

#[test]
fn test_buffer_push_increments_streak() {
    let mut buf = FailureBuffer::new(10);
    buf.push(make_entry(
        "llm_call",
        "mechanical",
        "429 Too Many Requests",
        Some("429"),
    ));
    buf.push(make_entry("parse", "model", "unparseable response", None));
    assert_eq!(buf.streak(), 2);
    assert_eq!(buf.entries().len(), 2);
}

#[test]
fn test_verified_resets_streak() {
    let mut buf = FailureBuffer::new(10);
    buf.push(make_entry("llm_call", "mechanical", "timeout", None));
    buf.push(make_entry("parse", "model", "bad json", None));
    assert_eq!(buf.streak(), 2);
    buf.reset();
    assert_eq!(buf.streak(), 0);
    assert_eq!(buf.entries().len(), 0);
}

#[test]
fn test_buffer_caps_at_max() {
    let mut buf = FailureBuffer::new(5);
    for i in 0..10 {
        buf.push(make_entry(
            "llm_call",
            "mechanical",
            &format!("error {}", i),
            None,
        ));
    }
    // Buffer capped at 5, but streak keeps counting
    assert_eq!(buf.entries().len(), 5);
    assert_eq!(buf.streak(), 10);
    // Oldest entries dropped — only the last 5 remain
    assert!(buf.entries()[0].reason.contains("5") || buf.entries()[0].reason.contains("6"));
}

#[test]
fn test_should_trigger_threshold() {
    let mut buf = FailureBuffer::new(10);
    assert!(!buf.should_trigger(2), "should not trigger with 0 failures");
    buf.push(make_entry("parse", "model", "error", None));
    assert!(!buf.should_trigger(2), "should not trigger with 1 failure");
    buf.push(make_entry("parse", "model", "error 2", None));
    assert!(buf.should_trigger(2), "should trigger at streak=2");
    buf.reset();
    assert!(!buf.should_trigger(2), "should not trigger after reset");
}

#[test]
fn test_only_reset_on_verified() {
    let mut buf = FailureBuffer::new(10);
    buf.push(make_entry(
        "validator_rejection",
        "model",
        "SymPy rejected",
        None,
    ));
    buf.push(make_entry("adversarial_veto", "model", "flaw found", None));
    assert_eq!(buf.streak(), 2);
    // Another failure does NOT reset
    buf.push(make_entry("parse", "model", "bad json", None));
    assert_eq!(buf.streak(), 3);
    // Only verified success resets
    buf.reset();
    assert_eq!(buf.streak(), 0);
}

// ---- parse_mid_run_verdict ----

#[test]
fn test_parse_valid_json() {
    let json = r#"{"classification":"mechanical","root_cause":"3x HTTP 429 from OpenAI in 45s","recommendation":"Switch to Anthropic or add backoff","confidence":0.95,"suggested_action":"add_backoff"}"#;
    let verdict = parse_mid_run_verdict(json).expect("should parse valid JSON");
    assert_eq!(verdict.classification, "mechanical");
    assert_eq!(verdict.suggested_action, "add_backoff");
    assert_eq!(verdict.root_cause, "3x HTTP 429 from OpenAI in 45s");
    assert!((verdict.confidence - 0.95).abs() < 0.001);
}

#[test]
fn test_parse_json_with_markdown_fence() {
    let fenced = "Here is the classification:\n```json\n{\"classification\":\"model\",\"root_cause\":\"bad math\",\"recommendation\":\"retry with different approach\",\"confidence\":0.7,\"suggested_action\":\"rephrase_prompt\"}\n```\nThat's all.";
    let verdict = parse_mid_run_verdict(fenced);
    assert!(
        verdict.is_some(),
        "should parse JSON wrapped in markdown fences"
    );
    assert_eq!(verdict.unwrap().classification, "model");
}

#[test]
fn test_parse_invalid_json_returns_none() {
    assert!(parse_mid_run_verdict("this is definitely not json").is_none());
    assert!(parse_mid_run_verdict("").is_none());
    assert!(parse_mid_run_verdict("{}").is_none()); // missing required fields → None
}

// ---- build_mid_run_prompt ----

#[test]
fn test_build_prompt_contains_failure_reasons() {
    let mut buf = FailureBuffer::new(10);
    buf.push(make_entry(
        "llm_call",
        "mechanical",
        "rate limit hit on openai",
        Some("429"),
    ));
    buf.push(make_entry(
        "parse",
        "model",
        "invalid json in response body",
        None,
    ));
    let prompt = build_mid_run_prompt(&buf, "gpt-4o", "algebra", 5, "test-attempt-id");
    assert!(
        prompt.contains("rate limit hit on openai"),
        "prompt must contain first failure reason"
    );
    assert!(
        prompt.contains("invalid json in response body"),
        "prompt must contain second failure reason"
    );
}

#[test]
fn test_build_prompt_contains_model_name() {
    let mut buf = FailureBuffer::new(10);
    buf.push(make_entry("llm_call", "mechanical", "error", None));
    let prompt = build_mid_run_prompt(
        &buf,
        "claude-haiku-4-5-20251001",
        "combinatorics",
        3,
        "test",
    );
    assert!(
        prompt.contains("claude-haiku-4-5-20251001"),
        "prompt must name the solver model"
    );
}

#[test]
fn test_build_prompt_contains_streak_count() {
    let mut buf = FailureBuffer::new(10);
    buf.push(make_entry("parse", "model", "e1", None));
    buf.push(make_entry("parse", "model", "e2", None));
    buf.push(make_entry("parse", "model", "e3", None));
    let prompt = build_mid_run_prompt(&buf, "test-model", "algebra", 7, "attempt-123");
    assert!(
        prompt.contains('3'),
        "prompt must mention the streak count (3)"
    );
}

// ---- extract_http_status ----

#[test]
fn test_extract_http_status_429() {
    assert_eq!(
        extract_http_status("API error 429 Too Many Requests"),
        Some("429".to_string())
    );
    assert_eq!(extract_http_status("HTTP 429"), Some("429".to_string()));
    assert_eq!(extract_http_status("status=429"), Some("429".to_string()));
}

#[test]
fn test_extract_http_status_503() {
    assert_eq!(
        extract_http_status("HTTP 503 Service Unavailable"),
        Some("503".to_string())
    );
}

#[test]
fn test_extract_http_status_none_for_non_http() {
    assert_eq!(extract_http_status("connection refused"), None);
    assert_eq!(extract_http_status("timeout after 30s"), None);
    assert_eq!(extract_http_status("JSON parse error"), None);
}

// ---- DB roundtrip ----

#[test]
fn test_record_finding_roundtrip() {
    let db = chatdb::db::Database::new_in_memory().expect("in-memory DB must succeed");
    let problem = db
        .create_problem("test problem", "algebra", "test")
        .expect("create problem");
    let attempt_id = db
        .create_attempt(&problem.id, &["test-model".to_string()])
        .expect("create attempt");

    let finding = chatdb::db::discerner::DiscernerFinding {
        id: "test-finding-id-001".to_string(),
        attempt_id: attempt_id.clone(),
        step_number: 3,
        failure_streak: 2,
        failure_window: "[]".to_string(),
        classification: "mechanical".to_string(),
        root_cause: "rate limit from OpenAI".to_string(),
        recommendation: "add 5s backoff delay".to_string(),
        confidence: 0.9,
        suggested_action: "add_backoff".to_string(),
        discerner_model: "claude-haiku-4-5-20251001".to_string(),
        created_at: "2026-02-28T00:00:00Z".to_string(),
    };

    db.record_discerner_finding(&finding)
        .expect("record_discerner_finding must succeed");

    // Verify the row was inserted
    let conn = db.conn();
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM discerner_findings WHERE attempt_id = ?1 AND classification = 'mechanical'",
        rusqlite::params![&attempt_id],
        |row| row.get(0),
    ).expect("query must succeed");
    assert_eq!(count, 1, "exactly one finding should be recorded");
}
