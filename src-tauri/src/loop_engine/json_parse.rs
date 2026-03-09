//! Centralized JSON extraction from LLM responses.
//!
//! LLMs often wrap JSON in markdown fences, prose preamble, or trailing text.
//! This module provides a single generic helper that all parsers share so
//! hardening applies everywhere at once.

use serde::de::DeserializeOwned;

/// Extract and parse JSON from an LLM response string.
///
/// Tries, in order:
/// 1. Direct parse (response is pure JSON)
/// 2. Markdown fence extraction (```json ... ```)
/// 3. Outer brace carving (first `{` to last `}`)
///
/// Returns `None` if all strategies fail.
pub fn extract_json<T: DeserializeOwned>(response: &str) -> Option<T> {
    let trimmed = response.trim();

    // 1. Direct parse
    if let Ok(val) = serde_json::from_str::<T>(trimmed) {
        return Some(val);
    }

    // 2. Markdown fence: ```json\n...\n``` or ```\n...\n```
    if let Some(val) = try_markdown_fence::<T>(trimmed) {
        return Some(val);
    }

    // 3. Brace carving: first { to last }
    if let Some(val) = try_brace_carve::<T>(trimmed) {
        return Some(val);
    }

    None
}

/// Like `extract_json` but also tries parsing as a JSON array.
/// Used for responses that may return `[...]` instead of `{...}`.
pub fn extract_json_or_array<T: DeserializeOwned>(response: &str) -> Option<T> {
    // Try object extraction first
    if let Some(val) = extract_json::<T>(response) {
        return Some(val);
    }

    // Try bracket carving: first [ to last ]
    let trimmed = response.trim();
    if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            if start < end {
                if let Ok(val) = serde_json::from_str::<T>(&trimmed[start..=end]) {
                    return Some(val);
                }
            }
        }
    }

    None
}

fn try_markdown_fence<T: DeserializeOwned>(text: &str) -> Option<T> {
    let fence_start = text.find("```")?;
    let after_fence = &text[fence_start + 3..];
    let content_start = after_fence.find('\n')?;
    let content = &after_fence[content_start + 1..];
    let fence_end = content.find("```")?;
    let block = content[..fence_end].trim();
    serde_json::from_str::<T>(block).ok()
}

fn try_brace_carve<T: DeserializeOwned>(text: &str) -> Option<T> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if start >= end {
        return None;
    }
    serde_json::from_str::<T>(&text[start..=end]).ok()
}
