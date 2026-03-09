use serde::{Deserialize, Serialize};

/// A single finding from the post-attempt review.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReviewFinding {
    pub finding_type: String,
    pub summary: String,
    pub detail: String,
    #[serde(default)]
    pub target_agent: Option<String>,
    #[serde(default)]
    pub priority: String,
}

/// Full result of a post-attempt review.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReviewResult {
    #[serde(default)]
    pub findings: Vec<ReviewFinding>,
    #[serde(default)]
    pub conclusion_sound: bool,
    #[serde(default)]
    pub conclusion_confidence: f64,
    #[serde(default)]
    pub exploration_coverage: f64,
    #[serde(default)]
    pub missing_constructions: Vec<String>,
    #[serde(default)]
    pub training_label: String,
}

/// Build the post-attempt review prompt from the full proof trace.
pub fn build_review_prompt(
    problem: &str,
    steps: &[crate::models::proof::Step],
    known_answer: Option<&str>,
    research_context: &str,
) -> String {
    let verified_count = steps.iter().filter(|s| s.verified).count();
    let rejected_count = steps.iter().filter(|s| !s.verified).count();

    // Extract conclusion from last verified step if it looks like one
    let conclusion = steps
        .iter()
        .rev()
        .find(|s| s.verified && s.proposal_type == "conclusion")
        .map(|s| s.proposal_natural.as_str());

    let mut p = String::from(
        "You are a proof review council. Analyze this completed proof attempt. \
Your goal is to identify what the solver MISSED — not errors in what it did, \
but gaps in what it DIDN'T explore.\n\n\
The most important failure mode to detect: locally correct proofs that are globally \
incomplete because the solver tunnel-visioned on one technique family.\n\n",
    );

    // Inject enriched context (prior attempts, research, pattern stats)
    if !research_context.is_empty() {
        p.push_str(research_context);
        p.push('\n');
    }

    p.push_str(&format!("PROBLEM: {}\n", problem));
    p.push_str(&format!(
        "SOLVER'S ANSWER: {}\n",
        conclusion.unwrap_or("(no explicit conclusion — budget exhausted or stopped)")
    ));
    if let Some(known) = known_answer {
        p.push_str(&format!("KNOWN CORRECT ANSWER: {}\n", known));
        if conclusion.is_some() {
            p.push_str(
                "NOTE: Compare the solver's answer with the known answer. If they differ, \
                        identify specifically what construction or technique was missed.\n",
            );
        }
    }
    p.push('\n');

    p.push_str(&format!(
        "FULL TRACE ({} steps, {} verified, {} rejected):\n",
        steps.len(),
        verified_count,
        rejected_count
    ));
    for step in steps {
        let verdict = if step.verified {
            "VERIFIED"
        } else {
            "REJECTED"
        };
        p.push_str(&format!(
            "  Step {} [{}] ({}): {}\n",
            step.step_number, verdict, step.proposal_type, step.proposal_natural
        ));
        if let Some(formal) = &step.proposal_formal {
            p.push_str(&format!("    formal: {}\n", formal));
        }
        if let Some(reason) = &step.rejection_reason {
            p.push_str(&format!("    rejection: {}\n", reason));
        }
    }
    p.push('\n');

    p.push_str(
"Respond with ONLY a JSON object (no markdown fences):\n\
{\n\
  \"findings\": [\n\
    {\n\
      \"finding_type\": \"technique_gap|conclusion_error|exploration_narrow|death_spiral\",\n\
      \"summary\": \"one-line summary\",\n\
      \"detail\": \"detailed explanation of what was missed and why it matters\",\n\
      \"target_agent\": \"solver|scout|patterns\",\n\
      \"priority\": \"high|medium|low\"\n\
    }\n\
  ],\n\
  \"conclusion_sound\": true/false,\n\
  \"conclusion_confidence\": 0.0-1.0,\n\
  \"exploration_coverage\": 0.0-1.0,\n\
  \"missing_constructions\": [\"specific constructions or techniques the solver should have tried\"],\n\
  \"training_label\": \"correct|locally_sound_globally_incomplete|wrong_answer|budget_exhausted\"\n\
}");
    p
}

/// Parse a review response from LLM text.
pub fn parse_review(response: &str) -> Option<ReviewResult> {
    super::json_parse::extract_json(response)
}
