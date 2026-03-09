use crate::api::llm_client::LlmClient;
use crate::contracts::loop_events::{
    LoopThinkingEndPayload, LoopThinkingStartPayload, LoopTokenPayload,
};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

// ============================================================
// Mid-run Discerner: fires during a proof after 2 consecutive
// failures to classify them as mechanical vs. model errors.
// ============================================================

/// One failure event recorded in the mid-run buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureEntry {
    /// Step number at failure time (None if LLM call failed before step advanced).
    pub step_number: Option<u32>,
    /// RFC3339 millis timestamp.
    pub ts: String,
    /// "llm_call" | "parse" | "validator_rejection" | "adversarial_veto"
    pub failure_type: String,
    /// "mechanical" | "model" | "validator" | "gate"
    pub category: String,
    /// Raw error text or rejection_reason.
    pub reason: String,
    /// HTTP status code if a network/API error, e.g. "429", "503".
    pub http_status: Option<String>,
    /// Solver model that was being called at the time.
    pub model: String,
    /// Proposed natural language (for validator/adversarial rejections).
    pub proposal_natural: Option<String>,
}

/// Bounded ring buffer that tracks consecutive failures and the streak count.
/// Resets only when a step is verified (success path).
pub struct FailureBuffer {
    entries: Vec<FailureEntry>,
    streak: u32,
    capacity: usize,
}

impl FailureBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            streak: 0,
            capacity,
        }
    }

    /// Push a new failure entry. Increments streak. Drops oldest entry if at capacity.
    pub fn push(&mut self, entry: FailureEntry) {
        self.streak += 1;
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    /// Reset streak and clear buffer. Call this when a step is verified (success).
    pub fn reset(&mut self) {
        self.streak = 0;
        self.entries.clear();
    }

    /// Returns true when streak has reached the trigger threshold.
    pub fn should_trigger(&self, threshold: u32) -> bool {
        self.streak >= threshold
    }

    pub fn streak(&self) -> u32 {
        self.streak
    }
    pub fn entries(&self) -> &[FailureEntry] {
        &self.entries
    }
}

/// Output of the mid-run Discerner: an actionable classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidRunVerdict {
    /// "mechanical" | "model" | "gate" | "validator"
    pub classification: String,
    /// One sentence naming the root cause.
    pub root_cause: String,
    /// One sentence concrete recommendation.
    pub recommendation: String,
    /// Confidence in classification, 0.0–1.0.
    pub confidence: f64,
    /// "switch_model" | "add_backoff" | "retry" | "rephrase_prompt" | "continue"
    pub suggested_action: String,
}

const MID_RUN_SYSTEM_PROMPT: &str = "\
You are the Discerner — a real-time diagnostic classifier for a mathematical proof engine.

The engine has hit consecutive failures. Your job: classify the root cause and recommend \
an immediate action so the orchestrator does not burn retries on the wrong problem.

CLASSIFICATION TAXONOMY:
- \"mechanical\" — Infrastructure problem: API rate limit (429), network timeout, sidecar \
  unavailable, authentication error. The MATH may be correct. Retrying after a delay or \
  switching providers would help.
- \"model\" — LLM output is wrong: bad math, parse errors, repeated algebraic mistakes, \
  SymPy/Pint rejected the claim. The infrastructure is fine. A different prompt or model \
  approach would help.
- \"validator\" — The CAS tool is the problem: SymPy syntax error for a valid expression, \
  Lean timeout, Pint unit format issue. Math may be correct but unverifiable right now.
- \"gate\" — Obligation or answer gate keeps blocking: solver tries to conclude before \
  resolving obligations, or keeps producing a wrong final answer.

SUGGESTED ACTIONS:
- \"switch_model\" — Use a different provider or model family
- \"add_backoff\" — Wait before retrying (rate limit hit)
- \"retry\" — Retry immediately (transient network error)
- \"rephrase_prompt\" — Reformulate solver instructions; the model is confused
- \"continue\" — The streak is noise; proceed normally

Respond with ONLY valid JSON (no markdown fences, no prose before or after):
{\"classification\":\"...\",\"root_cause\":\"...\",\"recommendation\":\"...\",\"confidence\":0.0,\"suggested_action\":\"...\"}";

/// Build the Discerner prompt for a mid-run failure streak.
pub fn build_mid_run_prompt(
    buffer: &FailureBuffer,
    model_name: &str,
    domain: &str,
    step_number: u32,
    attempt_id: &str,
) -> String {
    let failure_lines = buffer
        .entries()
        .iter()
        .map(|e| {
            let http_info = e
                .http_status
                .as_deref()
                .map(|s| format!(" http_status={s}"))
                .unwrap_or_default();
            format!(
                "  [{}] type={} category={} reason=\"{}\"{}",
                e.ts, e.failure_type, e.category, e.reason, http_info
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "{system}\n\n\
         CONTEXT: {streak} consecutive failures at step {step_number} (attempt {attempt_id}).\n\
         Solver model: {model_name}. Problem domain: {domain}.\n\n\
         RECENT FAILURE WINDOW (oldest first):\n{failures}\n\n\
         Classify and recommend.",
        system = MID_RUN_SYSTEM_PROMPT,
        streak = buffer.streak(),
        step_number = step_number,
        attempt_id = attempt_id,
        model_name = model_name,
        domain = domain,
        failures = failure_lines,
    )
}

/// Parse a MidRunVerdict from an LLM response (handles markdown fences + brace carving).
pub fn parse_mid_run_verdict(response: &str) -> Option<MidRunVerdict> {
    super::json_parse::extract_json(response)
}

/// Extract an HTTP status code string from an error message.
/// Returns the first recognized HTTP error code found in the string, or None.
pub fn extract_http_status(error: &str) -> Option<String> {
    for code in &[
        "429", "503", "502", "500", "504", "401", "403", "408", "404",
    ] {
        if error.contains(code) {
            return Some((*code).to_string());
        }
    }
    None
}

/// Run the mid-run Discerner: classify a failure streak in real-time during a proof attempt.
pub async fn classify_mid_run(
    buffer: &FailureBuffer,
    model_name: &str,
    domain: &str,
    step_number: u32,
    attempt_id: &str,
    llm: &LlmClient,
    app: &tauri::AppHandle,
) -> Result<MidRunVerdict, String> {
    let prompt = build_mid_run_prompt(buffer, model_name, domain, step_number, attempt_id);

    let _ = app.emit(
        "loop:thinking_start",
        LoopThinkingStartPayload {
            step_number: Some(step_number),
            model: llm.model_name(),
            agent_role: Some("discerner".to_string()),
            obligation_id: None,
            review: None,
            manual: None,
        },
    );

    let app_clone = app.clone();
    let result = llm
        .complete_streaming(&prompt, move |chunk| {
            let _ = app_clone.emit(
                "loop:token",
                LoopTokenPayload {
                    text: chunk.to_string(),
                    agent_role: Some("discerner".to_string()),
                    obligation_id: None,
                },
            );
        })
        .await;

    let _ = app.emit(
        "loop:thinking_end",
        LoopThinkingEndPayload {
            obligation_id: None,
        },
    );

    let resp = result.map_err(|e| format!("Discerner LLM error: {e}"))?;
    parse_mid_run_verdict(&resp.text)
        .ok_or_else(|| "Discerner: failed to parse JSON verdict".to_string())
}

const SYSTEM_PROMPT: &str = "\
You are the Discerner — a meta-analyst for a mathematical proof engine.

You receive a log of failures from a proof attempt and a summary of what was tried. \
Classify the primary failure mode:

MECHANICAL — failure caused by infrastructure: API errors, timeouts, sidecar unavailable, \
JSON parse errors, validator crashes. The reasoning logic itself may have been sound; \
the attempt simply never got a fair evaluation.

LOGICAL — failure caused by flawed reasoning: wrong math, unsound proof steps, unresolved \
obligations, circular arguments, wrong approach entirely. Infrastructure worked fine.

MIXED — both contributed meaningfully.

Respond ONLY with valid JSON (no markdown fences, no extra text):
{
  \"kind\": \"mechanical\" | \"logical\" | \"mixed\",
  \"confidence\": <0.0–1.0>,
  \"mechanical_score\": <0.0–1.0>,
  \"logical_score\": <0.0–1.0>,
  \"reasoning\": \"<1–3 sentence explanation>\",
  \"retry_rec\": \"same\" | \"new_approach\" | \"restructure\"
}";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureClassification {
    pub kind: String,
    pub confidence: f32,
    pub mechanical_score: f32,
    pub logical_score: f32,
    pub reasoning: String,
    pub retry_rec: String,
}

/// Classify the failure mode of a proof attempt from its failure log.
///
/// `failures` — Vec of (proposal_type, rejection_reason) pairs from the attempt.
/// Returns a classification or an error string if the LLM call fails / JSON unparseable.
pub async fn classify_failure(
    failures: &[(String, String)],
    step_summary: &str,
    llm: &LlmClient,
    app: &tauri::AppHandle,
    attempt_id: &str,
) -> Result<FailureClassification, String> {
    if failures.is_empty() {
        return Ok(FailureClassification {
            kind: "logical".to_string(),
            confidence: 0.5,
            mechanical_score: 0.0,
            logical_score: 0.5,
            reasoning: "No failures recorded; attempt may have run out of steps.".to_string(),
            retry_rec: "new_approach".to_string(),
        });
    }

    // Format failures as a numbered list for the prompt
    let failure_list = failures
        .iter()
        .enumerate()
        .map(|(i, (ptype, reason))| format!("{}. [{}] {}", i + 1, ptype, reason))
        .collect::<Vec<_>>()
        .join("\n");

    let user_prompt = format!(
        "ATTEMPT SUMMARY: {}\n\nFAILURE LOG ({} failures):\n{}\n\nClassify this failure.",
        step_summary,
        failures.len(),
        failure_list,
    );

    let full_prompt = format!("{}\n\n{}", SYSTEM_PROMPT, user_prompt);

    let _ = app.emit(
        "loop:thinking_start",
        LoopThinkingStartPayload {
            step_number: None,
            model: llm.model_name(),
            agent_role: Some("discerner".to_string()),
            obligation_id: None,
            review: None,
            manual: None,
        },
    );

    let app_clone = app.clone();
    let result = llm
        .complete_streaming(&full_prompt, move |chunk| {
            let _ = app_clone.emit(
                "loop:token",
                LoopTokenPayload {
                    text: chunk.to_string(),
                    agent_role: Some("discerner".to_string()),
                    obligation_id: None,
                },
            );
        })
        .await;

    let _ = app.emit(
        "loop:thinking_end",
        LoopThinkingEndPayload {
            obligation_id: None,
        },
    );

    let resp = result.map_err(|e| format!("Discerner LLM error: {e}"))?;
    let text = resp.text.trim();

    // Extract JSON object
    let json_str = if let Some(start) = text.find('{') {
        text.get(start..=text.rfind('}').unwrap_or(text.len().saturating_sub(1)))
            .unwrap_or(text)
    } else {
        text
    };

    let parsed: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("Discerner JSON parse error: {e}"))?;

    let kind = parsed
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("logical")
        .to_string();
    let confidence = parsed
        .get("confidence")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5) as f32;
    let mechanical_score = parsed
        .get("mechanical_score")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;
    let logical_score = parsed
        .get("logical_score")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5) as f32;
    let reasoning = parsed
        .get("reasoning")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let retry_rec = parsed
        .get("retry_rec")
        .and_then(|v| v.as_str())
        .unwrap_or("new_approach")
        .to_string();

    tracing::info!(
        "Discerner [{}] verdict: {} (confidence={:.2}, mech={:.2}, logic={:.2}) → {}",
        attempt_id,
        kind,
        confidence,
        mechanical_score,
        logical_score,
        retry_rec
    );

    Ok(FailureClassification {
        kind,
        confidence,
        mechanical_score,
        logical_score,
        reasoning,
        retry_rec,
    })
}
