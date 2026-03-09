use regex::Regex;
use serde::Serialize;

/// A mathematical claim extracted from text via regex.
#[derive(Debug, Clone, Serialize)]
pub struct ExtractedClaim {
    /// The raw text that was matched.
    pub raw_text: String,
    /// A SymPy-friendly formal expression (best effort).
    pub formal: String,
    /// Where it was found: "stream", "natural", "reasoning", "formal".
    pub source: String,
    /// Character offset in the token buffer where this was found.
    pub offset: usize,
    /// The step number this claim belongs to (0 = pre-step streaming).
    pub step_number: u32,
}

/// Streaming claim monitor — accumulates tokens and continuously extracts
/// mathematical claims as they arrive. Runs regex on every new chunk so
/// claims are detected in real-time, not just at the end.
pub struct StreamMonitor {
    buffer: String,
    /// Claims already found (to avoid duplicates on subsequent chunks).
    found_formals: std::collections::HashSet<String>,
    /// All claims extracted so far.
    pub claims: Vec<ExtractedClaim>,
    /// Current step number context.
    step_number: u32,
    /// Byte offset of last scan position (avoid re-scanning old text).
    scan_offset: usize,
}

impl Default for StreamMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamMonitor {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            found_formals: std::collections::HashSet::new(),
            claims: Vec::new(),
            step_number: 0,
            scan_offset: 0,
        }
    }

    /// Set the current step number for provenance tracking.
    pub fn set_step(&mut self, step: u32) {
        self.step_number = step;
    }

    /// Feed a new token chunk into the monitor. Returns any NEW claims
    /// extracted from this chunk (incremental — won't re-return old claims).
    pub fn feed(&mut self, chunk: &str) -> Vec<ExtractedClaim> {
        self.buffer.push_str(chunk);

        self.scan_new()
    }

    /// Reset the buffer for a new step (keeps cross-step claims for consistency checking).
    pub fn reset_buffer(&mut self) {
        self.buffer.clear();
        self.scan_offset = 0;
    }

    /// Get all claims for a specific step.
    pub fn claims_for_step(&self, step: u32) -> Vec<&ExtractedClaim> {
        self.claims
            .iter()
            .filter(|c| c.step_number == step)
            .collect()
    }

    /// Scan new content since last scan_offset for claims.
    fn scan_new(&mut self) -> Vec<ExtractedClaim> {
        // We need some lookback for regex context — patterns might span the chunk boundary
        let lookback = 80;
        let mut start = self.scan_offset.saturating_sub(lookback);
        // Ensure start falls on a UTF-8 char boundary (multi-byte chars like ≤ span 2-4 bytes)
        while start > 0 && !self.buffer.is_char_boundary(start) {
            start -= 1;
        }
        let text = &self.buffer[start..];

        let mut new_claims = Vec::new();
        let regex_claims = run_extraction(text, "stream");

        for mut claim in regex_claims {
            claim.step_number = self.step_number;
            claim.offset += start; // adjust offset relative to full buffer
            if self.found_formals.insert(claim.formal.clone()) {
                new_claims.push(claim.clone());
                self.claims.push(claim);
            }
        }

        self.scan_offset = self.buffer.len();
        new_claims
    }
}

/// Core extraction logic — used by both streaming monitor and batch extraction.
fn run_extraction(text: &str, source: &str) -> Vec<ExtractedClaim> {
    let mut claims = Vec::new();
    if text.is_empty() {
        return claims;
    }

    // Pattern 1: explicit equations like "2n - 2 = 2(n-1)" or "n^2 - n = n(n-1)"
    let eq_re =
        Regex::new(r"(?i)([a-z0-9_()\[\]*\^/+\- ]{2,40})\s*=\s*([a-z0-9_()\[\]*\^/+\- ]{1,40})")
            .unwrap();

    // Pattern 2: "the answer is N" / "minimum is N" / "maximum is N" etc.
    let answer_re = Regex::new(
        r"(?i)(?:the\s+)?(?:answer|minimum|maximum|result|total|count|value|number)\s+(?:is|equals?|=)\s+(\d[\d,. ]*\d|\d+)"
    ).unwrap();

    // Pattern 3: standalone large numbers (4+ digits, potential answers)
    let big_number_re = Regex::new(r"\b(\d{4,})\b").unwrap();

    // Pattern 4: inequalities like "x >= 5", "f(n) <= 2n"
    let ineq_re = Regex::new(
        r"(?i)([a-z0-9_()\[\]*\^/+\- ]{2,30})\s*([<>]=?|≤|≥)\s*([a-z0-9_()\[\]*\^/+\- ]{1,30})",
    )
    .unwrap();

    // Pattern 5: "therefore X", "hence X", "so X", "thus X" — conclusion markers
    let conclusion_re = Regex::new(
        r"(?i)(?:therefore|hence|thus|so|which gives|yielding|we get|this means)\s+(.{5,60}?)(?:\.|,|$)"
    ).unwrap();

    // Extract equations
    for cap in eq_re.captures_iter(text) {
        let lhs = cap[1].trim();
        let rhs = cap[2].trim();
        if lhs.len() < 2 || rhs.is_empty() {
            continue;
        }
        // Skip JSON-like keys ("proposal_type" = "...")
        if lhs.contains('"') || rhs.contains('"') {
            continue;
        }
        // Skip step references like "Step 1 ="
        if lhs.to_lowercase().starts_with("step") {
            continue;
        }
        let formal = format!("{} = {}", lhs.replace('^', "**"), rhs.replace('^', "**"));
        claims.push(ExtractedClaim {
            raw_text: cap[0].to_string(),
            formal,
            source: source.to_string(),
            offset: cap.get(0).map(|m| m.start()).unwrap_or(0),
            step_number: 0,
        });
    }

    // Extract "the answer is N" patterns
    for cap in answer_re.captures_iter(text) {
        let number = cap[1].replace([',', ' '], "");
        claims.push(ExtractedClaim {
            raw_text: cap[0].to_string(),
            formal: number,
            source: source.to_string(),
            offset: cap.get(0).map(|m| m.start()).unwrap_or(0),
            step_number: 0,
        });
    }

    // Extract standalone large numbers
    for cap in big_number_re.captures_iter(text) {
        let num = &cap[1];
        if !claims
            .iter()
            .any(|c| c.formal == *num || c.raw_text.contains(num))
        {
            claims.push(ExtractedClaim {
                raw_text: cap[0].to_string(),
                formal: num.to_string(),
                source: source.to_string(),
                offset: cap.get(0).map(|m| m.start()).unwrap_or(0),
                step_number: 0,
            });
        }
    }

    // Extract inequalities
    for cap in ineq_re.captures_iter(text) {
        let lhs = cap[1].trim();
        let op = &cap[2];
        let rhs = cap[3].trim();
        if lhs.len() < 2 || rhs.is_empty() {
            continue;
        }
        if lhs.contains('"') || rhs.contains('"') {
            continue;
        }
        let formal = format!(
            "{} {} {}",
            lhs.replace('^', "**"),
            op,
            rhs.replace('^', "**")
        );
        if !claims.iter().any(|c| c.formal == formal) {
            claims.push(ExtractedClaim {
                raw_text: cap[0].to_string(),
                formal,
                source: source.to_string(),
                offset: cap.get(0).map(|m| m.start()).unwrap_or(0),
                step_number: 0,
            });
        }
    }

    // Extract conclusion markers
    for cap in conclusion_re.captures_iter(text) {
        let claim_text = cap[1].trim();
        if claim_text.len() < 5 {
            continue;
        }
        // Only keep if it contains something mathematical
        if claim_text.chars().any(|c| c.is_ascii_digit() || c == '=') {
            claims.push(ExtractedClaim {
                raw_text: cap[0].to_string(),
                formal: claim_text.to_string(),
                source: source.to_string(),
                offset: cap.get(0).map(|m| m.start()).unwrap_or(0),
                step_number: 0,
            });
        }
    }

    claims
}

/// Batch extract claims from completed natural/reasoning text.
/// Convenience wrapper for non-streaming use.
pub fn extract_claims(natural: &str, reasoning: Option<&str>) -> Vec<ExtractedClaim> {
    let mut all = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (text, source) in [(natural, "natural"), (reasoning.unwrap_or(""), "reasoning")] {
        for claim in run_extraction(text, source) {
            if seen.insert(claim.formal.clone()) {
                all.push(claim);
            }
        }
    }
    all
}

/// Check if an extracted numeric claim conflicts with the conclusion's answer.
pub fn check_answer_consistency(
    claims: &[ExtractedClaim],
    conclusion_formal: Option<&str>,
    conclusion_natural: &str,
) -> Option<String> {
    let conclusion_number = extract_number(conclusion_formal.unwrap_or(""))
        .or_else(|| extract_number(conclusion_natural));

    let conclusion_num = conclusion_number?;

    for claim in claims {
        if let Some(claim_num) = extract_number(&claim.formal) {
            if (claim_num - conclusion_num).abs() < 0.001 {
                continue;
            }
            if claim_num < 10.0 {
                continue;
            }
            return Some(format!(
                "Numeric inconsistency: '{}' (from {}) implies {} but conclusion claims {}",
                claim.raw_text, claim.source, claim_num, conclusion_num
            ));
        }
    }
    None
}

/// Try to extract the largest significant number from text.
fn extract_number(text: &str) -> Option<f64> {
    let num_re = Regex::new(r"(\d+(?:\.\d+)?)").unwrap();
    let mut largest: Option<f64> = None;
    for cap in num_re.captures_iter(text) {
        if let Ok(n) = cap[1].parse::<f64>() {
            if n >= 10.0 {
                largest = Some(match largest {
                    Some(prev) if prev > n => prev,
                    _ => n,
                });
            }
        }
    }
    largest
}
