//! Stream repetition guard — detects LLM runaway / token-loop errors mid-stream.
//!
//! When a model enters a repetition loop (e.g. `})})})}...` repeated hundreds
//! of times), this guard sets an `AtomicBool` abort flag.  The streaming
//! functions in `llm_client` check the flag after every `on_token` call and
//! terminate the stream early, returning `Err("stream_aborted:repetition_detected")`.
//!
//! Usage:
//! ```ignore
//! let guard = RepetitionDetector::new();
//! let abort  = guard.abort_flag();
//! let feeder = guard.make_feeder();
//! let on_tok = move |chunk: &str| { feeder(chunk); emit_to_ui(chunk); };
//! let result = llm.complete_streaming_abortable(prompt, on_tok, abort).await;
//! if let Err(ref e) = result {
//!     if e.starts_with("stream_aborted") {
//!         // guard.pattern() has the repeated pattern
//!     }
//! }
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// ── Tuning constants ─────────────────────────────────────────────────────────

/// Minimum length of the repeating pattern to look for (chars).
const MIN_PATTERN_LEN: usize = 2;
/// Maximum length of the repeating pattern to look for (chars).
const MAX_PATTERN_LEN: usize = 24;
/// How many consecutive repetitions trigger an abort.
/// 12 × 24 = 288 chars max before abort fires — well before 500+ char garbage walls.
const MIN_REPS: usize = 12;
/// Rolling buffer kept for detection. Older text is dropped once we exceed 2×.
const BUFFER_SIZE: usize = 512;

// ─────────────────────────────────────────────────────────────────────────────

/// Stateful mid-stream repetition detector.
///
/// Create one per LLM call.  Feed each token chunk via `feed()` (or use the
/// `Fn` closure returned by `make_feeder()` for `Fn`-bounded callbacks).
/// When repetition is detected, the internal `AtomicBool` is set to `true`.
#[derive(Debug)]
pub struct RepetitionDetector {
    buffer: String,
    detected: bool,
    detected_pattern: Option<String>,
    abort: Arc<AtomicBool>,
}

impl RepetitionDetector {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            detected: false,
            detected_pattern: None,
            abort: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Clone of the abort flag — pass this to `complete_streaming_abortable`.
    pub fn abort_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.abort)
    }

    /// Returns a closure that feeds chunks into a shared detector state.
    ///
    /// The returned closure is `Fn(&str) + Send + 'static` so it can be
    /// composed inside the `on_token` callback without needing `&mut self`.
    pub fn make_feeder(&self) -> impl Fn(&str) + Send + 'static {
        let state = Arc::new(Mutex::new(InnerState {
            buffer: String::new(),
            detected: false,
        }));
        let abort = Arc::clone(&self.abort);
        move |chunk: &str| {
            if let Ok(mut s) = state.lock() {
                if s.detected {
                    return;
                }
                s.buffer.push_str(chunk);
                if s.buffer.len() > BUFFER_SIZE * 2 {
                    let trim = s.buffer.len() - BUFFER_SIZE;
                    let safe_trim = s.buffer.ceil_char_boundary(trim);
                    s.buffer = s.buffer[safe_trim..].to_string();
                }
                if detect_in_bytes(s.buffer.as_bytes()).is_some() {
                    s.detected = true;
                    abort.store(true, Ordering::Relaxed);
                }
            }
        }
    }

    /// Feed a chunk directly (when you have `&mut self` access).
    pub fn feed(&mut self, chunk: &str) -> bool {
        if self.detected {
            return true;
        }
        self.buffer.push_str(chunk);
        if self.buffer.len() > BUFFER_SIZE * 2 {
            let trim = self.buffer.len() - BUFFER_SIZE;
            let safe_trim = self.buffer.ceil_char_boundary(trim);
            self.buffer = self.buffer[safe_trim..].to_string();
        }
        if let Some(pat) = detect_in_bytes(self.buffer.as_bytes()) {
            self.detected = true;
            self.detected_pattern = Some(pat);
            self.abort.store(true, Ordering::Relaxed);
            return true;
        }
        false
    }

    pub fn is_triggered(&self) -> bool {
        self.abort.load(Ordering::Relaxed)
    }

    pub fn pattern(&self) -> Option<&str> {
        self.detected_pattern.as_deref()
    }
}

impl Default for RepetitionDetector {
    fn default() -> Self {
        Self::new()
    }
}

// Internal state used by make_feeder's Arc<Mutex<…>>.
#[derive(Debug)]
struct InnerState {
    buffer: String,
    detected: bool,
}

// ── Detection algorithm ───────────────────────────────────────────────────────

/// Look for a repeating pattern of length 2–24 that appears ≥ MIN_REPS consecutive
/// times in the tail of `buf`.  Returns the pattern string if found.
fn detect_in_bytes(buf: &[u8]) -> Option<String> {
    for pat_len in MIN_PATTERN_LEN..=MAX_PATTERN_LEN {
        let needed = pat_len * MIN_REPS;
        if buf.len() < needed {
            continue;
        }
        let tail = &buf[buf.len() - needed..];
        let pattern = &tail[..pat_len];
        if (0..MIN_REPS).all(|i| &tail[i * pat_len..(i + 1) * pat_len] == pattern) {
            return Some(String::from_utf8_lossy(pattern).into_owned());
        }
    }
    None
}

/// Post-hoc check on a complete response string.
///
/// Use this as a backup when you already have the full text (e.g. adversary
/// or reviewer responses that don't use the abortable streaming path).
pub fn is_repetition_loop(text: &str) -> bool {
    if text.len() < MIN_PATTERN_LEN * MIN_REPS {
        return false;
    }
    let check_from = text.len().saturating_sub(BUFFER_SIZE);
    // Snap to a char boundary so we never slice inside a multi-byte UTF-8 char
    let safe_from = text.ceil_char_boundary(check_from);
    detect_in_bytes(&text.as_bytes()[safe_from..]).is_some()
}

/// Human-readable description of the detected pattern for diagnostics.
pub fn describe_repetition(text: &str) -> Option<String> {
    let check_from = text.len().saturating_sub(BUFFER_SIZE);
    let safe_from = text.ceil_char_boundary(check_from);
    let pat = detect_in_bytes(&text.as_bytes()[safe_from..])?;
    // Escape non-printable chars for display
    let escaped: String = pat
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() || c == ' ' {
                c
            } else {
                '·'
            }
        })
        .collect();
    Some(format!(
        "pattern {:?} ({} chars) repeated ≥{}× in last {} chars",
        escaped,
        pat.len(),
        MIN_REPS,
        text.len() - check_from,
    ))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_simple_repetition() {
        let garbage = "})}".repeat(20);
        assert!(is_repetition_loop(&garbage));
    }

    #[test]
    fn ignores_normal_text() {
        let normal = "The proof proceeds by induction. Let n be a natural number. \
                      Assume the claim holds for n, we show it holds for n+1.";
        assert!(!is_repetition_loop(normal));
    }

    #[test]
    fn detects_via_feeder() {
        let guard = RepetitionDetector::new();
        let feeder = guard.make_feeder();
        let chunk = "})}".repeat(15);
        feeder(&chunk);
        assert!(guard.is_triggered());
    }

    #[test]
    fn no_false_positive_on_short_text() {
        assert!(!is_repetition_loop("})}"));
    }
}
