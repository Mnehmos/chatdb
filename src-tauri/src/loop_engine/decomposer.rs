use crate::api::llm_client::{LlmClient, LlmTurn};
use crate::contracts::loop_events::{
    LoopDecompositionCompletePayload, LoopDecompositionStartPayload, LoopObligationOpenedPayload,
};
use crate::db::Database;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// Output from the decomposer LLM call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecompositionResult {
    pub problem_profile: ProblemProfile,
    pub obligations: Vec<ObligationSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemProfile {
    pub problem_type: String,
    pub key_objects: Vec<String>,
    pub likely_techniques: Vec<String>,
    pub difficulty_estimate: String,
    pub critical_insight: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObligationSpec {
    pub description: String,
    pub obligation_type: String,
    pub priority: f64,
    pub depends_on: Vec<usize>,
    pub budget: i32,
    pub satisfaction_criteria: String,
}

/// Run the initial decomposition before the proof loop starts.
/// Returns the parsed result and the decomposition session ID.
pub async fn run_initial_decomposition(
    db: &Database,
    llm: &LlmClient,
    attempt_id: &str,
    branch_id: i32,
    problem_statement: &str,
    problem_domain: &str,
    known_answer: Option<&str>,
    recon_briefing: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(String, DecompositionResult), String> {
    let prompt = build_decomposition_prompt(
        problem_statement,
        problem_domain,
        known_answer,
        recon_briefing,
    );

    tracing::info!("Decomposer: analyzing problem for obligation graph");
    let _ = app_handle.emit(
        "loop:decomposition_start",
        LoopDecompositionStartPayload {
            attempt_id: attempt_id.to_string(),
            trigger: "initial".to_string(),
            model: llm.model_name(),
        },
    );

    let messages = [serde_json::json!({"role": "user", "content": &prompt})];
    let turn = llm
        .complete_with_tools(&messages, &[], |_| {})
        .await
        .map_err(|e| format!("Decomposer LLM error: {}", e))?;
    let result = match turn {
        LlmTurn::Text(r) => r,
        LlmTurn::ToolUse { .. } => {
            return Err("Decomposer LLM error: unexpected tool call".to_string())
        }
    };

    let parsed = parse_decomposition_response(&result.text)?;

    // Record the decomposition session
    let graph_json = serde_json::to_string(&parsed.obligations).unwrap_or_default();
    let profile_json = serde_json::to_string(&parsed.problem_profile).unwrap_or_default();
    let decomp_id = db
        .record_decomposition(
            attempt_id,
            "initial",
            None,
            &profile_json,
            &graph_json,
            parsed.obligations.len() as i32,
            &llm.model_name(),
            result.tokens_in,
            result.tokens_out,
        )
        .map_err(|e| format!("DB error recording decomposition: {}", e))?;

    // Create obligations with dependency tracking
    // First pass: create all obligations, collecting their IDs
    let mut obligation_ids: Vec<String> = Vec::new();
    for spec in &parsed.obligations {
        // depends_on uses indices — we'll resolve to IDs after creation
        let ob_id = db
            .create_obligation_with_deps(
                attempt_id,
                branch_id,
                "decomposer",
                &spec.description,
                &spec.obligation_type,
                spec.priority,
                0.8,
                Some(0), // source_layer 0 = decomposer
                None,    // depends_on filled in second pass
                Some(&decomp_id),
                Some(&spec.satisfaction_criteria),
                spec.budget,
            )
            .map_err(|e| format!("DB error creating obligation: {}", e))?;
        obligation_ids.push(ob_id);
    }

    // Second pass: wire up depends_on references (index → ID)
    for (i, spec) in parsed.obligations.iter().enumerate() {
        if !spec.depends_on.is_empty() {
            let dep_ids: Vec<&str> = spec
                .depends_on
                .iter()
                .filter_map(|&idx| obligation_ids.get(idx).map(|s| s.as_str()))
                .collect();
            if !dep_ids.is_empty() {
                let deps_json = serde_json::to_string(&dep_ids).unwrap_or_default();
                let conn = db.conn();
                let _ = conn.execute(
                    "UPDATE obligations SET depends_on = ?1 WHERE id = ?2",
                    rusqlite::params![deps_json, obligation_ids[i]],
                );
            }
        }
    }

    // Emit frontend events for each created obligation
    for (i, spec) in parsed.obligations.iter().enumerate() {
        let _ = app_handle.emit(
            "loop:obligation_opened",
            LoopObligationOpenedPayload {
                id: obligation_ids[i].clone(),
                description: spec.description.clone(),
                obligation_type: spec.obligation_type.clone(),
                priority: spec.priority,
                decomposition_id: Some(decomp_id.clone()),
            },
        );
    }

    let _ = app_handle.emit(
        "loop:decomposition_complete",
        LoopDecompositionCompletePayload {
            attempt_id: attempt_id.to_string(),
            decomposition_id: decomp_id.clone(),
            obligations_created: parsed.obligations.len() as u32,
            problem_type: parsed.problem_profile.problem_type.clone(),
            model: llm.model_name(),
        },
    );

    // Record dag_event
    let _ = db.append_dag_event(
        attempt_id,
        "decomposition_complete",
        &serde_json::json!({
            "decomposition_id": &decomp_id,
            "obligations": parsed.obligations.len(),
            "trigger": "initial",
        })
        .to_string(),
        "decomposer",
    );

    tracing::info!(
        "Decomposer: created {} obligations (type: {})",
        parsed.obligations.len(),
        parsed.problem_profile.problem_type
    );

    Ok((decomp_id, parsed))
}

/// Run re-decomposition triggered by contradiction, failure, or discovery.
pub async fn run_redecomposition(
    db: &Database,
    llm: &LlmClient,
    attempt_id: &str,
    branch_id: i32,
    problem_statement: &str,
    problem_domain: &str,
    known_answer: Option<&str>,
    trigger: &str,
    trigger_context: &str,
    previous_decomp_id: &str,
    verified_chain: &[(String, u32, String, String, String)],
    app_handle: &tauri::AppHandle,
) -> Result<(String, DecompositionResult), String> {
    let chain_summary: Vec<String> = verified_chain
        .iter()
        .map(|(_, n, _, nat, formal)| format!("Step {}: {} [{}]", n, nat, formal))
        .collect();

    let prompt = build_redecomposition_prompt(
        problem_statement,
        problem_domain,
        known_answer,
        trigger,
        trigger_context,
        &chain_summary,
    );

    tracing::info!("Decomposer: re-decomposing (trigger: {})", trigger);
    let _ = app_handle.emit(
        "loop:decomposition_start",
        LoopDecompositionStartPayload {
            attempt_id: attempt_id.to_string(),
            trigger: trigger.to_string(),
            model: llm.model_name(),
        },
    );

    let messages = [serde_json::json!({"role": "user", "content": &prompt})];
    let turn = llm
        .complete_with_tools(&messages, &[], |_| {})
        .await
        .map_err(|e| format!("Re-decomposer LLM error: {}", e))?;
    let result = match turn {
        LlmTurn::Text(r) => r,
        LlmTurn::ToolUse { .. } => {
            return Err("Re-decomposer LLM error: unexpected tool call".to_string())
        }
    };

    let parsed = parse_decomposition_response(&result.text)?;

    // Supersede old obligations from the previous decomposition
    let superseded = db
        .supersede_decomposition_obligations(previous_decomp_id, "redecomposition")
        .unwrap_or(0);
    if superseded > 0 {
        tracing::info!(
            "Decomposer: superseded {} obligations from previous decomposition",
            superseded
        );
    }

    // Record new decomposition
    let graph_json = serde_json::to_string(&parsed.obligations).unwrap_or_default();
    let profile_json = serde_json::to_string(&parsed.problem_profile).unwrap_or_default();
    let decomp_id = db
        .record_decomposition(
            attempt_id,
            trigger,
            None,
            &profile_json,
            &graph_json,
            parsed.obligations.len() as i32,
            &llm.model_name(),
            result.tokens_in,
            result.tokens_out,
        )
        .map_err(|e| format!("DB error recording re-decomposition: {}", e))?;

    // Create new obligations
    let mut obligation_ids: Vec<String> = Vec::new();
    for spec in &parsed.obligations {
        let ob_id = db
            .create_obligation_with_deps(
                attempt_id,
                branch_id,
                "decomposer",
                &spec.description,
                &spec.obligation_type,
                spec.priority,
                0.8,
                Some(0),
                None,
                Some(&decomp_id),
                Some(&spec.satisfaction_criteria),
                spec.budget,
            )
            .map_err(|e| format!("DB error creating obligation: {}", e))?;
        obligation_ids.push(ob_id);
    }

    // Wire depends_on
    for (i, spec) in parsed.obligations.iter().enumerate() {
        if !spec.depends_on.is_empty() {
            let dep_ids: Vec<&str> = spec
                .depends_on
                .iter()
                .filter_map(|&idx| obligation_ids.get(idx).map(|s| s.as_str()))
                .collect();
            if !dep_ids.is_empty() {
                let deps_json = serde_json::to_string(&dep_ids).unwrap_or_default();
                let conn = db.conn();
                let _ = conn.execute(
                    "UPDATE obligations SET depends_on = ?1 WHERE id = ?2",
                    rusqlite::params![deps_json, obligation_ids[i]],
                );
            }
        }
    }

    // Emit events
    for (i, spec) in parsed.obligations.iter().enumerate() {
        let _ = app_handle.emit(
            "loop:obligation_opened",
            LoopObligationOpenedPayload {
                id: obligation_ids[i].clone(),
                description: spec.description.clone(),
                obligation_type: spec.obligation_type.clone(),
                priority: spec.priority,
                decomposition_id: Some(decomp_id.clone()),
            },
        );
    }

    let _ = app_handle.emit(
        "loop:decomposition_complete",
        LoopDecompositionCompletePayload {
            attempt_id: attempt_id.to_string(),
            decomposition_id: decomp_id.clone(),
            obligations_created: parsed.obligations.len() as u32,
            problem_type: parsed.problem_profile.problem_type.clone(),
            model: llm.model_name(),
        },
    );

    let _ = db.append_dag_event(
        attempt_id,
        "redecomposition_complete",
        &serde_json::json!({
            "decomposition_id": &decomp_id,
            "trigger": trigger,
            "obligations": parsed.obligations.len(),
            "superseded": superseded,
        })
        .to_string(),
        "decomposer",
    );

    Ok((decomp_id, parsed))
}

fn build_decomposition_prompt(
    problem: &str,
    domain: &str,
    known_answer: Option<&str>,
    recon_briefing: &str,
) -> String {
    let mut prompt = format!(
        r#"You are a strategic problem decomposer for mathematical proofs.

TASK: Analyze this problem and decompose it into a dependency-ordered obligation graph.
Each obligation is a specific subgoal that the proof solver must resolve.

PROBLEM: {problem}
DOMAIN: {domain}
"#
    );

    // Inject reconnaissance intelligence
    if let Some(answer) = known_answer {
        prompt.push_str(&format!(
            r#"
SUSPECTED ANSWER (from external reconnaissance — treat as a strong hypothesis, not proven fact):
External sources suggest the answer is: {answer}
Build your obligation graph around VERIFYING this answer.
Include a CASE_CHECK obligation early (priority 0.95, budget 8) to confirm this value
via small-case computation before committing the rest of the graph to it.
If the answer is a constant, your CONSTRUCT obligation should find constructions achieving
exactly this bound, and your BOUND obligation should prove this is optimal.
If the solver disproves this hypothesis, the system will trigger re-decomposition.

"#
        ));
    } else if !recon_briefing.is_empty() {
        prompt.push_str(&format!(
            r#"
RECONNAISSANCE (external search results — treat as hints, not certainties):
{recon_briefing}

Your obligation graph is PROVISIONAL. Include a mandatory checkpoint:
- Type: CASE_CHECK, priority: 0.95, budget: 8
- Description: "Verify target value is optimal via small-case computation and extremal search"
- This checkpoint must be resolved before SYNTHESIZE proceeds.

"#
        ));
    } else {
        prompt.push_str(
            r#"
No external intelligence available. Your graph is PROVISIONAL.
Include a checkpoint obligation (CASE_CHECK, priority 0.95, budget 8):
"Verify target value is optimal via small-case computation and extremal search"

"#,
        );
    }

    prompt.push_str(
        r#"OBLIGATION TYPES (use these exact strings):
- COUNT: Establish a cardinality or quantity
- CLASSIFY: Characterize a family of objects — cover all cases
- CONSTRUCT: Build an explicit example or configuration
- BOUND: Prove an inequality or extremal result
- IMPOSSIBILITY: Show something cannot exist — derive a contradiction
- CASE_CHECK: Verify a specific small case
- SYNTHESIZE: Combine partial results into a final answer

RULES:
1. Each obligation must be a CAS-verifiable subgoal where possible (an algebraic identity).
2. Priority: 0.0-1.0 (1.0 = must solve first, critical dependency).
3. depends_on: array of indices (0-based) of obligations that must be resolved first.
4. budget: number of solver steps allowed for this obligation (5-30).
5. satisfaction_criteria: what verified step would close this obligation.
6. Keep to 3-7 obligations. More = more overhead. Fewer = less strategic value.
7. Order by dependency: foundations first, synthesis last.

OUTPUT FORMAT (JSON only, no markdown):
{
  "problem_profile": {
    "problem_type": "<e.g. combinatorics, number_theory, geometry, algebra>",
    "key_objects": ["<main mathematical objects>"],
    "likely_techniques": ["<e.g. double_counting, pigeonhole, generating_functions>"],
    "difficulty_estimate": "<easy|medium|hard|competition>",
    "critical_insight": "<one-sentence key insight, or null>"
  },
  "obligations": [
    {
      "description": "<what must be proved>",
      "obligation_type": "<one of the types above>",
      "priority": 0.9,
      "depends_on": [],
      "budget": 10,
      "satisfaction_criteria": "<what verified equality/step closes this>"
    }
  ]
}"#,
    );
    prompt
}

fn build_redecomposition_prompt(
    problem: &str,
    domain: &str,
    known_answer: Option<&str>,
    trigger: &str,
    trigger_context: &str,
    verified_steps: &[String],
) -> String {
    let chain = if verified_steps.is_empty() {
        "None yet.".to_string()
    } else {
        verified_steps.join("\n  ")
    };

    let answer_section = if let Some(answer) = known_answer {
        format!(
            r#"
SUSPECTED ANSWER (from external reconnaissance — strong hypothesis):
External sources suggest the answer is: {answer}
Build your revised obligation graph around VERIFYING this answer.
Include early CASE_CHECK validation. If prior steps already disproved this, ignore it.
"#
        )
    } else {
        String::new()
    };

    format!(
        r#"You are a strategic problem decomposer for mathematical proofs.

TASK: RE-DECOMPOSE this problem. The previous obligation graph needs revision.

PROBLEM: {problem}
DOMAIN: {domain}
{answer_section}
TRIGGER: {trigger}
CONTEXT: {trigger_context}

VERIFIED STEPS SO FAR:
  {chain}

Based on the trigger and progress so far, create a REVISED obligation graph.
Keep obligations that are still relevant. Add new ones needed. Remove obsolete ones.
Account for what has already been verified — don't re-derive established facts.

Use the same output format as initial decomposition:
{{
  "problem_profile": {{ ... }},
  "obligations": [ {{ ... }} ]
}}

OUTPUT: JSON only, no markdown."#
    )
}

fn parse_decomposition_response(text: &str) -> Result<DecompositionResult, String> {
    // Try direct parse
    if let Ok(result) = serde_json::from_str::<DecompositionResult>(text) {
        return Ok(result);
    }

    // Try extracting JSON from markdown fences
    let json_str = if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            &text[start..=end]
        } else {
            text
        }
    } else {
        text
    };

    serde_json::from_str::<DecompositionResult>(json_str).map_err(|e| {
        format!(
            "Failed to parse decomposer response: {}. Text: {}",
            e,
            &text[..text.len().min(200)]
        )
    })
}
