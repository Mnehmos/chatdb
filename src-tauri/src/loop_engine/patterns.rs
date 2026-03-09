use serde::Deserialize;

/// A pattern extracted from a verified proof chain.
#[derive(Debug, Clone, Deserialize)]
pub struct ExtractedPattern {
    pub name: String,
    pub description: String,
    pub trigger_text: String,
    pub strategy: String,
    #[serde(default)]
    pub technique_class: String,
    #[serde(default)]
    pub source_step_numbers: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct ExtractionResponse {
    patterns: Vec<ExtractedPattern>,
}

/// Build the pattern extraction prompt from a completed proof.
pub fn build_extraction_prompt(
    problem: &str,
    domain: &str,
    verified_steps: &[crate::models::proof::Step],
) -> String {
    let mut p = String::from(
"You are a mathematical technique librarian. Extract reusable technique patterns \
from this verified proof so they can help solve FUTURE problems.\n\n\
RULES:\n\
- Extract 1-3 patterns per proof. Fewer is better than vague.\n\
- Each pattern must be GENERAL — applicable to other problems, not a restatement of this specific proof.\n\
- trigger_text: what in a problem statement suggests this technique applies (e.g. \"expression of form a²-b²\")\n\
- strategy: step-by-step instructions another solver could follow (e.g. \"1. Identify a,b 2. Factor as (a-b)(a+b) 3. Verify by expansion\")\n\
- technique_class: category like \"factorization\", \"substitution\", \"induction\", \"telescoping\", \"generating_function\"\n\
- source_step_numbers: which verified step numbers demonstrate this technique\n\n");

    p.push_str(&format!("PROBLEM: {}\n", problem));
    p.push_str(&format!("DOMAIN: {}\n\n", domain));

    p.push_str(&format!(
        "VERIFIED PROOF CHAIN ({} steps):\n",
        verified_steps.len()
    ));
    for step in verified_steps {
        p.push_str(&format!(
            "  Step {} [{}]: {}\n",
            step.step_number, step.proposal_type, step.proposal_natural
        ));
        if let Some(formal) = &step.proposal_formal {
            p.push_str(&format!("    formal: {}\n", formal));
        }
        if let Some(reasoning) = &step.proposal_reasoning {
            p.push_str(&format!("    reasoning: {}\n", reasoning));
        }
    }
    p.push('\n');

    p.push_str(
        "Respond with ONLY a JSON object (no markdown fences):\n\
{\n\
  \"patterns\": [\n\
    {\n\
      \"name\": \"short descriptive name\",\n\
      \"description\": \"what this technique does and when it's useful\",\n\
      \"trigger_text\": \"what in a problem suggests using this technique\",\n\
      \"strategy\": \"step-by-step instructions to apply this technique\",\n\
      \"technique_class\": \"category\",\n\
      \"source_step_numbers\": [1, 2, 3]\n\
    }\n\
  ]\n\
}",
    );
    p
}

/// Parse an extraction response from LLM text.
pub fn parse_extraction(response: &str) -> Option<Vec<ExtractedPattern>> {
    // Try as ExtractionResponse wrapper first, then bare array
    if let Some(r) = super::json_parse::extract_json::<ExtractionResponse>(response) {
        return Some(r.patterns);
    }
    super::json_parse::extract_json_or_array::<Vec<ExtractedPattern>>(response)
}
