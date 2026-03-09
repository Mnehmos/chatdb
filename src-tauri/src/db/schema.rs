pub const SCHEMA_V2: &str = r##"
CREATE TABLE IF NOT EXISTS problems (
    id              TEXT PRIMARY KEY,
    statement       TEXT NOT NULL,
    formal_statement TEXT,
    domain          TEXT,
    source          TEXT,
    status          TEXT DEFAULT 'open',
    created_at      TEXT NOT NULL,
    solved_at       TEXT,
    total_attempts  INTEGER DEFAULT 0,
    total_steps     INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS attempts (
    id              TEXT PRIMARY KEY,
    problem_id      TEXT NOT NULL REFERENCES problems(id),
    strategy        TEXT,
    status          TEXT DEFAULT 'active',
    started_at      TEXT NOT NULL,
    ended_at        TEXT,
    step_count      INTEGER DEFAULT 0,
    backtrack_count INTEGER DEFAULT 0,
    cost_tokens_in  INTEGER DEFAULT 0,
    cost_tokens_out INTEGER DEFAULT 0,
    models_used     TEXT
);

CREATE TABLE IF NOT EXISTS steps (
    id                  TEXT PRIMARY KEY,
    attempt_id          TEXT NOT NULL REFERENCES attempts(id),
    parent_step_id      TEXT,
    step_number         INTEGER NOT NULL,
    model               TEXT NOT NULL,
    context_refs        TEXT,
    goal_state          TEXT NOT NULL,
    context_provided    TEXT,
    proposal_type       TEXT NOT NULL,
    proposal_natural    TEXT NOT NULL,
    proposal_formal     TEXT,
    proposal_reasoning  TEXT,
    sympy_result        TEXT,
    sympy_passed        INTEGER,
    pint_result         TEXT,
    pint_passed         INTEGER,
    lean_result         TEXT,
    lean_passed         INTEGER,
    critic_prediction   TEXT,
    critic_reasoning    TEXT,
    verified            INTEGER NOT NULL,
    rejection_reason    TEXT,
    model_tokens_in     INTEGER,
    model_tokens_out    INTEGER,
    wall_time_ms        INTEGER,
    created_at          TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS patterns (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    description     TEXT NOT NULL,
    domain          TEXT,
    trigger_text    TEXT NOT NULL,
    strategy        TEXT NOT NULL,
    source_steps    TEXT NOT NULL,
    success_count   INTEGER DEFAULT 1,
    failure_count   INTEGER DEFAULT 0,
    avg_steps       REAL,
    technique_class TEXT,
    deprecated      INTEGER DEFAULT 0,
    deprecated_by   TEXT,
    created_at      TEXT NOT NULL,
    last_used_at    TEXT
);

CREATE TABLE IF NOT EXISTS modifications (
    id              TEXT PRIMARY KEY,
    target_system   TEXT NOT NULL,
    description     TEXT NOT NULL,
    code_diff       TEXT NOT NULL,
    triggered_by    TEXT,
    meta_verified   INTEGER NOT NULL,
    active          INTEGER DEFAULT 0,
    created_at      TEXT NOT NULL,
    activated_at    TEXT
);

CREATE TABLE IF NOT EXISTS orchestrator_decisions (
    id              TEXT PRIMARY KEY,
    attempt_id      TEXT,
    problem_id      TEXT,
    decision_type   TEXT NOT NULL,
    proof_state     TEXT,
    worker_states   TEXT,
    resource_state  TEXT,
    decision        TEXT NOT NULL,
    reasoning       TEXT,
    outcome         TEXT,
    steps_to_next_verify INTEGER,
    cost_after      INTEGER,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS critic_evaluations (
    id              TEXT PRIMARY KEY,
    step_id         TEXT NOT NULL,
    attempt_id      TEXT NOT NULL,
    prediction      TEXT NOT NULL,
    confidence      REAL,
    reasoning       TEXT NOT NULL,
    actual_outcome  INTEGER,
    prediction_correct INTEGER,
    cost_saved      INTEGER,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS council_sessions (
    id              TEXT PRIMARY KEY,
    trigger_type    TEXT NOT NULL,
    problem_id      TEXT NOT NULL,
    attempt_id      TEXT,
    council_models  TEXT NOT NULL,
    moderator_model TEXT,
    transcript      TEXT NOT NULL,
    findings_count  INTEGER DEFAULT 0,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS council_findings (
    id              TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL REFERENCES council_sessions(id),
    finding_type    TEXT NOT NULL,
    summary         TEXT NOT NULL,
    detail          TEXT NOT NULL,
    step_refs       TEXT,
    consensus       TEXT NOT NULL,
    dissent         TEXT,
    target_agent    TEXT,
    acted_on        INTEGER DEFAULT 0,
    impact_notes    TEXT,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS scout_queries (
    id              TEXT PRIMARY KEY,
    trigger_type    TEXT NOT NULL,
    trigger_ref     TEXT,
    query_text      TEXT NOT NULL,
    sources_queried TEXT NOT NULL,
    results_count   INTEGER DEFAULT 0,
    results_summary TEXT,
    techniques_found TEXT,
    injected_into   TEXT,
    helped          INTEGER,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS librarian_actions (
    id              TEXT PRIMARY KEY,
    action_type     TEXT NOT NULL,
    trigger_ref     TEXT,
    pattern_id      TEXT,
    reasoning       TEXT NOT NULL,
    impact_measured INTEGER DEFAULT 0,
    solver_performance_delta REAL,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS research_cache (
    id              TEXT PRIMARY KEY,
    source          TEXT NOT NULL,
    query_hash      TEXT NOT NULL,
    query_text      TEXT NOT NULL,
    response_json   TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    last_accessed   TEXT NOT NULL,
    access_count    INTEGER DEFAULT 1,
    ttl_hours       INTEGER DEFAULT 168
);

CREATE TABLE IF NOT EXISTS branches (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    attempt_id      TEXT NOT NULL REFERENCES attempts(id),
    parent_branch   INTEGER DEFAULT 0,
    fork_step       INTEGER,
    fork_reason     TEXT,
    direction       TEXT,
    status          TEXT DEFAULT 'active',
    conclusion      TEXT,
    conclusion_value TEXT,
    step_count      INTEGER DEFAULT 0,
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_steps_attempt ON steps(attempt_id, step_number);
CREATE INDEX IF NOT EXISTS idx_steps_model ON steps(model, verified);
CREATE INDEX IF NOT EXISTS idx_patterns_domain ON patterns(domain, success_count DESC);
CREATE INDEX IF NOT EXISTS idx_attempts_problem ON attempts(problem_id, status);
CREATE INDEX IF NOT EXISTS idx_council_problem ON council_sessions(problem_id);
CREATE INDEX IF NOT EXISTS idx_findings_session ON council_findings(session_id);
CREATE INDEX IF NOT EXISTS idx_critic_step ON critic_evaluations(step_id);
CREATE INDEX IF NOT EXISTS idx_branches_attempt ON branches(attempt_id);
"##;

/// Migrations that add columns to existing tables.
/// Each ALTER TABLE is wrapped in a closure that ignores "duplicate column" errors,
/// so this is safe to run on both fresh and existing databases.
pub const MIGRATIONS_V3: &[&str] = &[
    "ALTER TABLE steps ADD COLUMN step_type TEXT DEFAULT 'proof'",
    "ALTER TABLE steps ADD COLUMN branch_id INTEGER DEFAULT 0",
    "ALTER TABLE steps ADD COLUMN technique_class TEXT",
    "ALTER TABLE problems ADD COLUMN known_answer TEXT",
];

/// V5 migrations: Challenge/adversarial data on steps table.
/// Each ALTER TABLE ignores "duplicate column" errors (safe to re-run).
pub const MIGRATIONS_V5: &[&str] = &[
    "ALTER TABLE steps ADD COLUMN challenge_model TEXT",
    "ALTER TABLE steps ADD COLUMN challenge_flaw_found INTEGER",
    "ALTER TABLE steps ADD COLUMN challenge_attack TEXT",
    "ALTER TABLE steps ADD COLUMN challenge_confidence REAL",
    "ALTER TABLE steps ADD COLUMN challenge_fatal INTEGER",
];

/// V4 migrations: DAG architecture — proof_nodes, obligations, dag_events, technique_registry.
/// Uses CREATE TABLE IF NOT EXISTS so safe to run repeatedly.
pub const MIGRATIONS_V4: &str = r##"
CREATE TABLE IF NOT EXISTS proof_nodes (
    id                  TEXT PRIMARY KEY,
    attempt_id          TEXT NOT NULL,
    branch_id           INTEGER DEFAULT 0,
    node_type           TEXT NOT NULL,
    parent_ids          TEXT,
    content             TEXT NOT NULL,
    formal_content      TEXT,
    technique_class     TEXT,
    construction_family TEXT,
    status              TEXT DEFAULT 'proposed',
    validator_used      TEXT,
    validator_result    TEXT,
    model_id            TEXT,
    obligation_ref      TEXT,
    opens_obligations   TEXT,
    step_id             TEXT,
    token_cost          INTEGER,
    sequence_number     INTEGER NOT NULL,
    created_at          TEXT NOT NULL,
    verified_at         TEXT
);
CREATE INDEX IF NOT EXISTS idx_nodes_attempt ON proof_nodes(attempt_id, sequence_number);
CREATE INDEX IF NOT EXISTS idx_nodes_type ON proof_nodes(attempt_id, node_type);
CREATE INDEX IF NOT EXISTS idx_nodes_step ON proof_nodes(step_id);

CREATE TABLE IF NOT EXISTS obligations (
    id                  TEXT PRIMARY KEY,
    attempt_id          TEXT NOT NULL,
    branch_id           INTEGER DEFAULT 0,
    parent_node_id      TEXT NOT NULL,
    description         TEXT NOT NULL,
    obligation_type     TEXT NOT NULL,
    priority            REAL DEFAULT 0.5,
    confidence          REAL DEFAULT 0.7,
    source_layer        INTEGER,
    status              TEXT DEFAULT 'open',
    assigned_model      TEXT,
    closure_node_id     TEXT,
    closure_type        TEXT,
    escalation_level    INTEGER DEFAULT 0,
    steps_spent         INTEGER DEFAULT 0,
    max_steps           INTEGER DEFAULT 20,
    search_space        TEXT,
    superseded_by       TEXT,
    retraction_reason   TEXT,
    closure_note        TEXT,
    created_at          TEXT NOT NULL,
    closed_at           TEXT
);
CREATE INDEX IF NOT EXISTS idx_obligations_attempt ON obligations(attempt_id, status);
CREATE INDEX IF NOT EXISTS idx_obligations_parent ON obligations(parent_node_id);

CREATE TABLE IF NOT EXISTS dag_events (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    attempt_id          TEXT NOT NULL,
    event_type          TEXT NOT NULL,
    payload             TEXT NOT NULL,
    agent_role          TEXT NOT NULL,
    sequence_number     INTEGER NOT NULL,
    created_at          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_dag_events_attempt ON dag_events(attempt_id, sequence_number);

CREATE TABLE IF NOT EXISTS technique_registry (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    problem_class       TEXT NOT NULL,
    technique_family    TEXT NOT NULL,
    description         TEXT NOT NULL,
    source              TEXT DEFAULT 'seed',
    success_count       INTEGER DEFAULT 0,
    failure_count       INTEGER DEFAULT 0,
    last_used_at        TEXT,
    created_at          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_techniques_class ON technique_registry(problem_class);
"##;

/// V6: Agent profile storage — persists full MultiAgentConfig as JSON.
/// Uses CREATE TABLE IF NOT EXISTS so safe to run repeatedly.
pub const MIGRATIONS_V6: &str = r##"
CREATE TABLE IF NOT EXISTS agent_profiles (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT,
    config_json TEXT NOT NULL,
    is_default  INTEGER DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_profiles_default ON agent_profiles(is_default);
"##;

/// V7: Discerner decisions — records failure classification verdicts (mechanical vs. logical).
pub const MIGRATIONS_V7: &str = r##"
CREATE TABLE IF NOT EXISTS discerner_decisions (
    id               TEXT PRIMARY KEY,
    attempt_id       TEXT NOT NULL,
    kind             TEXT NOT NULL,
    confidence       REAL NOT NULL,
    mechanical_score REAL NOT NULL,
    logical_score    REAL NOT NULL,
    reasoning        TEXT NOT NULL,
    retry_rec        TEXT NOT NULL,
    model            TEXT NOT NULL,
    failures_json    TEXT NOT NULL,
    created_at       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_discerner_attempt ON discerner_decisions(attempt_id);
"##;

/// V8: Mid-run discerner findings — records in-flight failure classifications
/// (fires during proof after 2 consecutive failures, not just post-attempt).
pub const MIGRATIONS_V8: &str = r##"
CREATE TABLE IF NOT EXISTS discerner_findings (
    id               TEXT PRIMARY KEY,
    attempt_id       TEXT NOT NULL,
    step_number      INTEGER NOT NULL,
    failure_streak   INTEGER NOT NULL,
    failure_window   TEXT NOT NULL,
    classification   TEXT NOT NULL,
    root_cause       TEXT NOT NULL,
    recommendation   TEXT NOT NULL,
    confidence       REAL NOT NULL,
    suggested_action TEXT NOT NULL,
    discerner_model  TEXT NOT NULL,
    created_at       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_discerner_findings_attempt ON discerner_findings(attempt_id, step_number);
"##;

/// V9: Decomposer — strategic problem decomposition into typed obligation graphs.
/// Adds dependency tracking to obligations and a decompositions session table.
pub const MIGRATIONS_V9: &str = r##"
CREATE TABLE IF NOT EXISTS decompositions (
    id                TEXT PRIMARY KEY,
    attempt_id        TEXT NOT NULL,
    trigger           TEXT NOT NULL,
    trigger_node_id   TEXT,
    problem_profile   TEXT NOT NULL,
    obligation_graph  TEXT NOT NULL,
    obligations_created INTEGER NOT NULL,
    model_used        TEXT NOT NULL,
    tokens_in         INTEGER,
    tokens_out        INTEGER,
    created_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_decompositions_attempt ON decompositions(attempt_id);
"##;

/// V9 column migrations for obligations table (ALTER TABLE, ignore if exists).
pub const MIGRATIONS_V9_COLS: &[&str] = &[
    "ALTER TABLE obligations ADD COLUMN depends_on TEXT",
    "ALTER TABLE obligations ADD COLUMN decomposition_id TEXT",
    "ALTER TABLE obligations ADD COLUMN satisfaction_criteria TEXT",
];

/// V10: Management System — claims, dag_edges, conflicts, after_action_reports.
/// Adds structured claim registry, unified DAG relationship index,
/// contradiction detection, and persistent post-mortem reports.
pub const MIGRATIONS_V10: &str = r##"
CREATE TABLE IF NOT EXISTS claims (
    id                TEXT PRIMARY KEY,
    step_id           TEXT NOT NULL,
    attempt_id        TEXT NOT NULL,
    claim_type        TEXT NOT NULL,
    object            TEXT NOT NULL,
    scope_type        TEXT,
    scope_param       TEXT,
    scope_constraint  TEXT,
    direction         TEXT,
    value             TEXT,
    natural_text      TEXT NOT NULL,
    confidence        REAL DEFAULT 1.0,
    verified          INTEGER DEFAULT 0,
    superseded_by     TEXT,
    created_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_claims_step ON claims(step_id);
CREATE INDEX IF NOT EXISTS idx_claims_attempt ON claims(attempt_id);
CREATE INDEX IF NOT EXISTS idx_claims_type ON claims(claim_type, object);

CREATE TABLE IF NOT EXISTS dag_edges (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id         TEXT NOT NULL,
    target_id         TEXT NOT NULL,
    source_type       TEXT NOT NULL,
    target_type       TEXT NOT NULL,
    edge_type         TEXT NOT NULL,
    metadata          TEXT,
    created_at        TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_edges_unique ON dag_edges(source_id, target_id, edge_type);
CREATE INDEX IF NOT EXISTS idx_edges_source ON dag_edges(source_id, edge_type);
CREATE INDEX IF NOT EXISTS idx_edges_target ON dag_edges(target_id, edge_type);
CREATE INDEX IF NOT EXISTS idx_edges_type ON dag_edges(edge_type);

CREATE TABLE IF NOT EXISTS conflicts (
    id                TEXT PRIMARY KEY,
    attempt_id        TEXT NOT NULL,
    claim_a_id        TEXT NOT NULL,
    claim_b_id        TEXT NOT NULL,
    conflict_type     TEXT NOT NULL,
    severity          TEXT NOT NULL,
    description       TEXT NOT NULL,
    resolution        TEXT,
    resolution_step   TEXT,
    resolved_at       TEXT,
    created_at        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_conflicts_attempt ON conflicts(attempt_id);
CREATE INDEX IF NOT EXISTS idx_conflicts_claims ON conflicts(claim_a_id, claim_b_id);

CREATE TABLE IF NOT EXISTS after_action_reports (
    id                  TEXT PRIMARY KEY,
    attempt_id          TEXT NOT NULL UNIQUE,
    problem_id          TEXT NOT NULL,
    coverage            REAL,
    soundness           REAL,
    budget_efficiency   REAL,
    death_spirals       INTEGER DEFAULT 0,
    contradictions      INTEGER DEFAULT 0,
    obligations_total   INTEGER DEFAULT 0,
    obligations_closed  INTEGER DEFAULT 0,
    findings_json       TEXT,
    recommendations     TEXT,
    training_label      TEXT,
    created_at          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_aar_attempt ON after_action_reports(attempt_id);
CREATE INDEX IF NOT EXISTS idx_aar_problem ON after_action_reports(problem_id);
"##;

/// V10 column migrations: extend problems, attempts, steps, obligations.
pub const MIGRATIONS_V10_COLS: &[&str] = &[
    // problems: add title, difficulty, metadata
    "ALTER TABLE problems ADD COLUMN title TEXT",
    "ALTER TABLE problems ADD COLUMN difficulty TEXT",
    "ALTER TABLE problems ADD COLUMN metadata TEXT",
    // attempts: add budget/coverage/efficiency tracking
    "ALTER TABLE attempts ADD COLUMN attempt_number INTEGER",
    "ALTER TABLE attempts ADD COLUMN mode TEXT DEFAULT 'obligation'",
    "ALTER TABLE attempts ADD COLUMN budget_total INTEGER",
    "ALTER TABLE attempts ADD COLUMN budget_used INTEGER DEFAULT 0",
    "ALTER TABLE attempts ADD COLUMN steps_verified INTEGER DEFAULT 0",
    "ALTER TABLE attempts ADD COLUMN steps_rejected INTEGER DEFAULT 0",
    "ALTER TABLE attempts ADD COLUMN coverage REAL",
    "ALTER TABLE attempts ADD COLUMN efficiency REAL",
    "ALTER TABLE attempts ADD COLUMN inherited_from TEXT",
    "ALTER TABLE attempts ADD COLUMN config TEXT",
    // steps: add obligation FK, denormalized obligation info, formal_lean, approach tracking
    "ALTER TABLE steps ADD COLUMN obligation_id TEXT",
    "ALTER TABLE steps ADD COLUMN obligation_desc TEXT",
    "ALTER TABLE steps ADD COLUMN obligation_type TEXT",
    "ALTER TABLE steps ADD COLUMN formal_lean TEXT",
    "ALTER TABLE steps ADD COLUMN approach TEXT",
    "ALTER TABLE steps ADD COLUMN failure_class TEXT",
    "ALTER TABLE steps ADD COLUMN generator_model TEXT",
    // obligations: add budget tracking, blacklist, metadata
    "ALTER TABLE obligations ADD COLUMN budget_total INTEGER",
    "ALTER TABLE obligations ADD COLUMN budget_used INTEGER DEFAULT 0",
    "ALTER TABLE obligations ADD COLUMN approach_blacklist TEXT",
    "ALTER TABLE obligations ADD COLUMN ob_metadata TEXT",
];

/// V11: Research API key storage — secure, DB-backed key management for research tools.
pub const MIGRATIONS_V11: &str = r##"
CREATE TABLE IF NOT EXISTS research_api_keys (
    id          TEXT PRIMARY KEY,
    service     TEXT NOT NULL UNIQUE,
    key_value   TEXT NOT NULL,
    label       TEXT,
    active      INTEGER DEFAULT 1,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_research_keys_service ON research_api_keys(service, active);
"##;

/// V13: Contrastive pairs — when a step is rejected at the same slot as a verified step,
/// record the pair for fine-tuning training data (rejected vs accepted, same position).
pub const MIGRATIONS_V13: &str = r##"
CREATE TABLE IF NOT EXISTS contrastive_pairs (
    id                  TEXT PRIMARY KEY,
    attempt_id          TEXT NOT NULL,
    step_number         INTEGER NOT NULL,
    accepted_step_id    TEXT NOT NULL,
    rejected_step_id    TEXT NOT NULL,
    rejection_reason    TEXT,
    created_at          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_contrastive_attempt ON contrastive_pairs(attempt_id);
CREATE INDEX IF NOT EXISTS idx_contrastive_step ON contrastive_pairs(step_number, attempt_id);
"##;

/// V12: Obligation satisfaction signals — tally-based obligation closure.
/// Multiple agents (mechanical, solver, reviewer, adversary) vote on whether
/// an obligation is satisfied. Majority of touches → close.
pub const MIGRATIONS_V12: &str = r##"
CREATE TABLE IF NOT EXISTS obligation_signals (
    id              TEXT PRIMARY KEY,
    obligation_id   TEXT NOT NULL,
    step_id         TEXT,
    source          TEXT NOT NULL,
    model_id        TEXT,
    satisfies       INTEGER NOT NULL,
    confidence      REAL DEFAULT 1.0,
    note            TEXT,
    created_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_ob_signals_obligation ON obligation_signals(obligation_id);
CREATE INDEX IF NOT EXISTS idx_ob_signals_step ON obligation_signals(step_id);
"##;

/// V14: Tool-Use Redesign — tool_runs, tool_artifacts, scout_sessions,
/// obligation_resolutions, resolution_corpus, corpus_usages, tool_policy_violations.
/// Makes tool-use a first-class part of the proof graph with persistent verified
/// resolution corpus and obligation-level scout passes.
pub const MIGRATIONS_V14: &str = r##"
CREATE TABLE IF NOT EXISTS tool_runs (
    id                  TEXT PRIMARY KEY,
    attempt_id          TEXT NOT NULL,
    branch_id           INTEGER,
    step_id             TEXT,
    step_number         INTEGER,
    obligation_id       TEXT,
    parent_tool_run_id  TEXT,
    session_id          TEXT,
    agent_role          TEXT NOT NULL,
    trigger_kind        TEXT NOT NULL,
    tool_name           TEXT NOT NULL,
    provider            TEXT NOT NULL,
    tier                INTEGER,
    query_json          TEXT NOT NULL,
    input_hash          TEXT,
    required_by_policy  INTEGER DEFAULT 0,
    status              TEXT NOT NULL,
    hit                 INTEGER,
    latency_ms          INTEGER,
    error_message       TEXT,
    raw_result_json     TEXT,
    result_summary      TEXT,
    created_at          TEXT NOT NULL,
    completed_at        TEXT
);
CREATE INDEX IF NOT EXISTS idx_tool_runs_attempt ON tool_runs(attempt_id, created_at);
CREATE INDEX IF NOT EXISTS idx_tool_runs_obligation ON tool_runs(obligation_id, created_at);
CREATE INDEX IF NOT EXISTS idx_tool_runs_session ON tool_runs(session_id, tool_name);

CREATE TABLE IF NOT EXISTS tool_artifacts (
    id                  TEXT PRIMARY KEY,
    tool_run_id         TEXT NOT NULL,
    attempt_id          TEXT NOT NULL,
    obligation_id       TEXT,
    artifact_index      INTEGER NOT NULL,
    artifact_kind       TEXT NOT NULL,
    external_ref        TEXT,
    title               TEXT,
    statement           TEXT,
    formalism           TEXT,
    url                 TEXT,
    relevance_score     REAL,
    matched             INTEGER DEFAULT 0,
    raw_json            TEXT NOT NULL,
    created_at          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tool_artifacts_run ON tool_artifacts(tool_run_id, artifact_index);

CREATE TABLE IF NOT EXISTS scout_sessions (
    id                      TEXT PRIMARY KEY,
    attempt_id              TEXT NOT NULL,
    branch_id               INTEGER,
    problem_id              TEXT NOT NULL,
    obligation_id           TEXT,
    scope                   TEXT NOT NULL,
    trigger_type            TEXT NOT NULL,
    routing_plan_json       TEXT NOT NULL,
    status                  TEXT NOT NULL,
    best_resolution_status  TEXT,
    best_tool_run_id        TEXT,
    best_artifact_id        TEXT,
    confidence              REAL,
    created_at              TEXT NOT NULL,
    completed_at            TEXT
);
CREATE INDEX IF NOT EXISTS idx_scout_sessions_obligation ON scout_sessions(obligation_id, created_at);
CREATE INDEX IF NOT EXISTS idx_scout_sessions_attempt ON scout_sessions(attempt_id, created_at);

CREATE TABLE IF NOT EXISTS obligation_resolutions (
    id                      TEXT PRIMARY KEY,
    attempt_id              TEXT NOT NULL,
    obligation_id           TEXT NOT NULL,
    scout_session_id        TEXT NOT NULL,
    status                  TEXT NOT NULL,
    source_tier             INTEGER NOT NULL,
    source_provider         TEXT NOT NULL,
    source_reference        TEXT NOT NULL,
    source_url              TEXT,
    source_formalism        TEXT NOT NULL,
    mapping_text            TEXT NOT NULL,
    translation_json        TEXT,
    mechanical_verification INTEGER DEFAULT 0,
    confidence              REAL NOT NULL,
    corpus_entry_id         TEXT,
    created_at              TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_obligation_resolutions_obligation ON obligation_resolutions(obligation_id, created_at);

CREATE TABLE IF NOT EXISTS resolution_corpus (
    id                      TEXT PRIMARY KEY,
    canonical_statement     TEXT NOT NULL,
    canonical_hash          TEXT NOT NULL,
    obligation_type         TEXT NOT NULL,
    problem_domain          TEXT,
    signature_json          TEXT NOT NULL,
    embedding_json          TEXT,
    source_tier             INTEGER NOT NULL,
    source_provider         TEXT NOT NULL,
    source_reference        TEXT NOT NULL,
    source_url              TEXT,
    source_formalism        TEXT NOT NULL,
    mapping_text            TEXT NOT NULL,
    translation_json        TEXT NOT NULL,
    mechanical_verification INTEGER DEFAULT 0,
    verification_method     TEXT,
    lean_proof_term         TEXT,
    tags_json               TEXT,
    reuse_count             INTEGER DEFAULT 0,
    last_reused_at          TEXT,
    active                  INTEGER DEFAULT 1,
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_resolution_corpus_hash ON resolution_corpus(canonical_hash, active);

CREATE TABLE IF NOT EXISTS corpus_usages (
    id                  TEXT PRIMARY KEY,
    corpus_entry_id     TEXT NOT NULL,
    attempt_id          TEXT NOT NULL,
    problem_id          TEXT NOT NULL,
    obligation_id       TEXT NOT NULL,
    scout_session_id    TEXT NOT NULL,
    resolution_id       TEXT,
    created_at          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_corpus_usages_entry ON corpus_usages(corpus_entry_id, created_at);

CREATE TABLE IF NOT EXISTS tool_policy_violations (
    id                      TEXT PRIMARY KEY,
    attempt_id              TEXT NOT NULL,
    branch_id               INTEGER,
    step_id                 TEXT,
    step_number             INTEGER,
    obligation_id           TEXT,
    agent_role              TEXT NOT NULL,
    policy_name             TEXT NOT NULL,
    required_tool           TEXT NOT NULL,
    observed_tool_runs_json TEXT,
    action_taken            TEXT NOT NULL,
    notes                   TEXT,
    created_at              TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tool_policy_violations_attempt ON tool_policy_violations(attempt_id, created_at);
"##;

/// V14 column migrations: extend obligations with scout metadata, steps with tool policy fields.
pub const MIGRATIONS_V14_COLS: &[&str] = &[
    // obligations: scout metadata
    "ALTER TABLE obligations ADD COLUMN signature_json TEXT",
    "ALTER TABLE obligations ADD COLUMN embedding_json TEXT",
    "ALTER TABLE obligations ADD COLUMN scout_status TEXT",
    "ALTER TABLE obligations ADD COLUMN last_scout_session_id TEXT",
    "ALTER TABLE obligations ADD COLUMN last_scout_confidence REAL",
    "ALTER TABLE obligations ADD COLUMN resolved_externally INTEGER DEFAULT 0",
    "ALTER TABLE obligations ADD COLUMN resolved_by_corpus_id TEXT",
    "ALTER TABLE obligations ADD COLUMN external_reference TEXT",
    "ALTER TABLE obligations ADD COLUMN scout_last_checked_at TEXT",
    // steps: tool policy tracking
    "ALTER TABLE steps ADD COLUMN solver_round_id TEXT",
    "ALTER TABLE steps ADD COLUMN tool_call_count INTEGER DEFAULT 0",
    "ALTER TABLE steps ADD COLUMN sympy_preflight_used INTEGER DEFAULT 0",
    "ALTER TABLE steps ADD COLUMN tool_policy_status TEXT",
];

/// V15 column migrations: fan-in solver collaboration metadata.
pub const MIGRATIONS_V15_COLS: &[&str] = &[
    // steps: fan-in metadata
    "ALTER TABLE steps ADD COLUMN solver_worker_id TEXT",
    "ALTER TABLE steps ADD COLUMN solver_dispatch_mode TEXT",
    "ALTER TABLE steps ADD COLUMN stale_sibling INTEGER DEFAULT 0",
    // obligations: collaborative assignment
    "ALTER TABLE obligations ADD COLUMN assigned_models_json TEXT",
    "ALTER TABLE obligations ADD COLUMN active_solver_round_id TEXT",
];

/// V16 column migrations: training-data quality labels.
pub const MIGRATIONS_V16_COLS: &[&str] =
    &["ALTER TABLE steps ADD COLUMN semantic_redundant INTEGER DEFAULT 0"];
