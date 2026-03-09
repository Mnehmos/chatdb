use crate::api::llm_client::{ToolCall, ToolDef};
use crate::api::sidecar::{ResearchSearchRequest, SidecarClient};

/// Return all tool definitions for LLM tool-use: SymPy pre-check + 7 research tools.
pub fn tool_definitions() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "sympy_check".into(),
            description: "Verify that your proposed formal expression is algebraically correct BEFORE submitting. \
Pass the left and right sides of your formal equality separately. \
Returns {is_equal: true} if correct, or {is_equal: false, diff: \"...<symbolic diff>...\"} showing the algebraic discrepancy. \
Call this every time you write a formal field. Fix any errors before submitting your JSON.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "lhs": {
                        "type": "string",
                        "description": "Left-hand side of the proposed formal equality (SymPy syntax, no = sign)"
                    },
                    "rhs": {
                        "type": "string",
                        "description": "Right-hand side of the proposed formal equality (SymPy syntax, no = sign)"
                    }
                },
                "required": ["lhs", "rhs"]
            }),
        },
        ToolDef {
            name: "claim_check".into(),
            description: "Verify a typed claim (divisibility, inequality, gcd, congruence, for_all) BEFORE submitting. \
Use this instead of sympy_check when your claim is not a pure equality. \
Returns {verified: true} or {verified: false, reason: \"...\"}. \
Example: {\"type\": \"divisibility\", \"dividend\": \"b**a\", \"divisor\": \"a\"}".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "claim": {
                        "type": "object",
                        "description": "Typed claim object with 'type' field and type-specific fields",
                        "properties": {
                            "type": {"type": "string", "enum": ["divisibility", "inequality", "gcd", "congruence", "for_all"]},
                            "dividend": {"type": "string"}, "divisor": {"type": "string"},
                            "lhs": {"type": "string"}, "rhs": {"type": "string"}, "relation": {"type": "string"},
                            "a": {"type": "string"}, "b": {"type": "string"}, "value": {"type": "string"},
                            "expr": {"type": "string"}, "remainder": {"type": "string"}, "modulus": {"type": "string"},
                            "variable": {"type": "string"}, "domain": {"type": "string"}, "predicate": {"type": "string"}
                        },
                        "required": ["type"]
                    }
                },
                "required": ["claim"]
            }),
        },
        ToolDef {
            name: "arxiv_search".into(),
            description: "Search arXiv for math papers, theorems, and techniques.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query for arXiv papers"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default 3)",
                        "default": 3
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDef {
            name: "oeis_lookup".into(),
            description: "Search OEIS integer sequences by terms or description.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query — sequence terms (e.g. '1,1,2,3,5,8') or description"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default 3)",
                        "default": 3
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDef {
            name: "semantic_scholar_search".into(),
            description: "Search academic papers with citation data.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query for academic papers"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default 3)",
                        "default": 3
                    },
                    "year": {
                        "type": "string",
                        "description": "Filter by publication year (e.g. '2023' or '2020-2024')"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDef {
            name: "wolfram_query".into(),
            description: "Query Wolfram Alpha for computation or symbolic math.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Wolfram Alpha query (computation, symbolic math, etc.)"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDef {
            name: "tavily_search".into(),
            description: "AI-optimized web search with extracted page content. Returns relevant snippets and an AI-generated answer. \
Use this for general math research, finding solutions to similar problems, or looking up theorems and techniques not covered by arXiv/OEIS.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (natural language, can be long)"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default 5)",
                        "default": 5
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDef {
            name: "loogle_search".into(),
            description: "Search Lean 4 Mathlib by type signature or name.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Type signature or name to search in Mathlib"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDef {
            name: "get_paper".into(),
            description: "Fetch full details of a specific paper by ID.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "enum": ["arxiv", "semantic_scholar"],
                        "description": "Paper source — 'arxiv' or 'semantic_scholar'"
                    },
                    "id": {
                        "type": "string",
                        "description": "Paper ID (e.g. '2301.00001' for arXiv)"
                    }
                },
                "required": ["source", "id"]
            }),
        },
        ToolDef {
            name: "get_sequence".into(),
            description: "Fetch full details of an OEIS sequence by ID.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "OEIS sequence ID (e.g. 'A000108')"
                    }
                },
                "required": ["id"]
            }),
        },
        // --- Lean 4 REPL tools (kernel-verified reasoning scratch pad) ---
        ToolDef {
            name: "lean_check".into(),
            description: "Kernel-verify an algebraic equality using Lean 4 with Mathlib. \
Pass LHS and RHS separately (Lean syntax: ^ for powers, * for multiplication). \
Returns {passed: true} if the Lean proof kernel confirms the equality, or {passed: false, error: \"...\"} with the kernel error. \
This is a PROOF — if Lean says it's true, it's mathematically certain. Use this to verify key steps.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "lhs": {
                        "type": "string",
                        "description": "Left-hand side (Lean 4 syntax, e.g. 'x^2 - 1')"
                    },
                    "rhs": {
                        "type": "string",
                        "description": "Right-hand side (Lean 4 syntax, e.g. '(x-1)*(x+1)')"
                    },
                    "variables": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Free variables (e.g. ['x', 'y']). Omit for numeric checks."
                    }
                },
                "required": ["lhs", "rhs"]
            }),
        },
        ToolDef {
            name: "lean_cmd".into(),
            description: "Send a Lean 4 command to the persistent REPL scratch pad. \
Use this to define helper lemmas, explore types with #check, or build incremental proofs. \
The returned env id can be passed back for subsequent commands that build on this definition. \
Example: 'def myHelper (n : Nat) := n * (n + 1) / 2' or '#check Nat.add_comm'.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "cmd": {
                        "type": "string",
                        "description": "Lean 4 command (theorem, def, #check, etc.)"
                    },
                    "env": {
                        "type": "integer",
                        "description": "Environment id from a previous lean_cmd result. Omit to use base (Mathlib) env."
                    }
                },
                "required": ["cmd"]
            }),
        },
        ToolDef {
            name: "lean_tactic".into(),
            description: "Send a tactic to close a proof goal in the Lean REPL. \
First create a theorem with 'sorry' via lean_cmd to get a proofState id, \
then send tactics to close goals step by step. \
Returns remaining goals — empty goals means the proof is complete.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "tactic": {
                        "type": "string",
                        "description": "Lean 4 tactic to apply (e.g. 'ring', 'omega', 'induction n', 'exact rfl')"
                    },
                    "proof_state": {
                        "type": "integer",
                        "description": "Proof state id from a previous lean_cmd sorry or lean_tactic result"
                    }
                },
                "required": ["tactic", "proof_state"]
            }),
        },
    ]
}

/// Execute a research tool call against the sidecar.
/// Returns a formatted string result (never panics).
pub async fn execute_tool_call(sidecar: &SidecarClient, call: &ToolCall) -> String {
    match call.name.as_str() {
        "sympy_check" => {
            let lhs = call.input.get("lhs").and_then(|v| v.as_str()).unwrap_or("");
            let rhs = call.input.get("rhs").and_then(|v| v.as_str()).unwrap_or("");
            match sidecar.sympy_check(lhs, rhs).await {
                Some(resp) => {
                    if resp.is_equal {
                        "EQUAL — formal expression is algebraically correct. Safe to submit.".into()
                    } else if resp.error.as_deref() == Some("abstract_function_skip") {
                        "SKIPPED — formal contains abstract function application (f(n), g(x), etc.). \
Cannot verify symbolically but the validator will accept it. Safe to submit.".into()
                    } else {
                        let diff = resp.diff.as_deref().unwrap_or("(unknown)");
                        format!(
                            "NOT EQUAL — algebraic error detected.\n\
Diff (LHS − RHS): {}\n\
Your formal expression does not hold as an identity. Fix it before submitting.",
                            diff
                        )
                    }
                }
                None => "sympy_check unavailable (sidecar timeout or error). Proceed with caution."
                    .into(),
            }
        }
        "claim_check" => {
            if let Some(claim) = call.input.get("claim") {
                match sidecar.claim_check(claim).await {
                    Some(resp) => {
                        if resp.verified {
                            format!(
                                "VERIFIED — typed claim ({}) is correct. Safe to submit.",
                                claim.get("type").and_then(|t| t.as_str()).unwrap_or("?")
                            )
                        } else {
                            format!(
                                "NOT VERIFIED — claim check failed.\nReason: {}\nFix the claim before submitting.",
                                resp.reason
                            )
                        }
                    }
                    None => {
                        "claim_check unavailable (sidecar timeout or error). Proceed with caution."
                            .into()
                    }
                }
            } else {
                "Error: missing 'claim' object in tool input.".into()
            }
        }
        "arxiv_search" => {
            let query = call
                .input
                .get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("");
            let max_results = call
                .input
                .get("max_results")
                .and_then(|m| m.as_u64())
                .unwrap_or(3) as u32;
            let req = ResearchSearchRequest {
                source: "arxiv".into(),
                query: query.into(),
                max_results: Some(max_results),
                sort: None,
                year: None,
                categories: None,
                fields_of_study: None,
                task: None,
                format_type: None,
                db: None,
            };
            match sidecar.research_search(&req).await {
                Ok(val) => format_results("arxiv_search", &val),
                Err(e) => format!("Error: {}", e),
            }
        }
        "oeis_lookup" => {
            let query = call
                .input
                .get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("");
            let max_results = call
                .input
                .get("max_results")
                .and_then(|m| m.as_u64())
                .unwrap_or(3) as u32;
            let req = ResearchSearchRequest {
                source: "oeis".into(),
                query: query.into(),
                max_results: Some(max_results),
                sort: None,
                year: None,
                categories: None,
                fields_of_study: None,
                task: None,
                format_type: None,
                db: None,
            };
            match sidecar.research_search(&req).await {
                Ok(val) => format_results("oeis_lookup", &val),
                Err(e) => format!("Error: {}", e),
            }
        }
        "semantic_scholar_search" => {
            let query = call
                .input
                .get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("");
            let max_results = call
                .input
                .get("max_results")
                .and_then(|m| m.as_u64())
                .unwrap_or(3) as u32;
            let year = call
                .input
                .get("year")
                .and_then(|y| y.as_str())
                .map(|y| y.to_string());
            let req = ResearchSearchRequest {
                source: "semantic_scholar".into(),
                query: query.into(),
                max_results: Some(max_results),
                sort: None,
                year,
                categories: None,
                fields_of_study: None,
                task: None,
                format_type: None,
                db: None,
            };
            match sidecar.research_search(&req).await {
                Ok(val) => format_results("semantic_scholar_search", &val),
                Err(e) => format!("Error: {}", e),
            }
        }
        "wolfram_query" => {
            let query = call
                .input
                .get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("");
            let req = ResearchSearchRequest {
                source: "wolfram".into(),
                query: query.into(),
                max_results: None,
                sort: None,
                year: None,
                categories: None,
                fields_of_study: None,
                task: None,
                format_type: Some("short".into()),
                db: None,
            };
            match sidecar.research_search(&req).await {
                Ok(val) => format_results("wolfram_query", &val),
                Err(e) => format!("Error: {}", e),
            }
        }
        "tavily_search" => {
            let query = call
                .input
                .get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("");
            let max_results = call
                .input
                .get("max_results")
                .and_then(|m| m.as_u64())
                .unwrap_or(5) as u32;
            let req = ResearchSearchRequest {
                source: "tavily".into(),
                query: query.into(),
                max_results: Some(max_results),
                sort: None,
                year: None,
                categories: None,
                fields_of_study: None,
                task: None,
                format_type: None,
                db: None,
            };
            match sidecar.research_search(&req).await {
                Ok(val) => format_results("tavily_search", &val),
                Err(e) => format!("Error: {}", e),
            }
        }
        "loogle_search" => {
            let query = call
                .input
                .get("query")
                .and_then(|q| q.as_str())
                .unwrap_or("");
            let req = ResearchSearchRequest {
                source: "loogle".into(),
                query: query.into(),
                max_results: None,
                sort: None,
                year: None,
                categories: None,
                fields_of_study: None,
                task: None,
                format_type: None,
                db: None,
            };
            match sidecar.research_search(&req).await {
                Ok(val) => format_results("loogle_search", &val),
                Err(e) => format!("Error: {}", e),
            }
        }
        "get_paper" => {
            let source = call
                .input
                .get("source")
                .and_then(|s| s.as_str())
                .unwrap_or("arxiv");
            let id = call.input.get("id").and_then(|i| i.as_str()).unwrap_or("");
            match sidecar.research_get(source, id).await {
                Ok(val) => format_results(source, &val),
                Err(e) => format!("Error: {}", e),
            }
        }
        "get_sequence" => {
            let id = call.input.get("id").and_then(|i| i.as_str()).unwrap_or("");
            match sidecar.research_get("oeis", id).await {
                Ok(val) => format_results("oeis", &val),
                Err(e) => format!("Error: {}", e),
            }
        }
        // --- Lean 4 REPL tools ---
        "lean_check" => {
            let lhs = call.input.get("lhs").and_then(|v| v.as_str()).unwrap_or("");
            let rhs = call.input.get("rhs").and_then(|v| v.as_str()).unwrap_or("");
            let variables: Vec<&str> = call
                .input
                .get("variables")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            match sidecar.lean_check(lhs, rhs, &variables).await {
                Some(resp) => {
                    if resp.passed {
                        format!(
                            "LEAN VERIFIED ✓ — the equality {} = {} is mathematically proven. \
Wall time: {}ms.",
                            lhs, rhs, resp.wall_time_ms
                        )
                    } else {
                        format!(
                            "LEAN REJECTED ✗ — the equality {} = {} failed kernel verification.\n\
Error: {}\n\
The expression may be wrong or may need a different type annotation.",
                            lhs,
                            rhs,
                            resp.error.as_deref().unwrap_or("(no details)")
                        )
                    }
                }
                None => "lean_check unavailable (REPL not ready or sidecar timeout). \
Use sympy_check as fallback."
                    .into(),
            }
        }
        "lean_cmd" => {
            let cmd = call.input.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
            let env = call.input.get("env").and_then(|v| v.as_i64());
            match sidecar.lean_cmd(cmd, env).await {
                Some(resp) => {
                    let mut out = String::new();
                    if resp.has_errors {
                        out.push_str("LEAN ERROR:\n");
                    }
                    for msg in &resp.messages {
                        let sev = msg
                            .get("severity")
                            .and_then(|s| s.as_str())
                            .unwrap_or("info");
                        let data = msg.get("data").and_then(|d| d.as_str()).unwrap_or("");
                        out.push_str(&format!("[{}] {}\n", sev, data));
                    }
                    if !resp.sorries.is_empty() {
                        out.push_str(&format!(
                            "\n{} sorry(s) — use lean_tactic to close:\n",
                            resp.sorries.len()
                        ));
                        for s in &resp.sorries {
                            if let Some(goal) = s.get("goal").and_then(|g| g.as_str()) {
                                let ps = s.get("proofState").and_then(|p| p.as_i64()).unwrap_or(-1);
                                out.push_str(&format!("  proofState={}: {}\n", ps, goal));
                            }
                        }
                    }
                    if let Some(e) = resp.env {
                        out.push_str(&format!(
                            "\nenv={} (use this id for subsequent lean_cmd calls)",
                            e
                        ));
                    }
                    out
                }
                None => "lean_cmd unavailable (REPL not ready or sidecar timeout).".into(),
            }
        }
        "lean_tactic" => {
            let tactic = call
                .input
                .get("tactic")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let proof_state = call
                .input
                .get("proof_state")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            match sidecar.lean_tactic(tactic, proof_state).await {
                Some(resp) => {
                    if resp.is_complete {
                        format!(
                            "PROOF COMPLETE ✓ — tactic '{}' closed all goals. \
The theorem is now proven.",
                            tactic
                        )
                    } else if resp.goals.is_empty() && !resp.messages.is_empty() {
                        let errors: Vec<String> = resp
                            .messages
                            .iter()
                            .filter_map(|m| {
                                m.get("data").and_then(|d| d.as_str()).map(String::from)
                            })
                            .collect();
                        format!(
                            "TACTIC FAILED — '{}' did not apply.\nErrors: {}",
                            tactic,
                            errors.join("; ")
                        )
                    } else {
                        let mut out = format!(
                            "Tactic '{}' applied. {} goal(s) remaining:\n",
                            tactic,
                            resp.goals.len()
                        );
                        for (i, goal) in resp.goals.iter().enumerate() {
                            out.push_str(&format!("  Goal {}: {}\n", i + 1, goal));
                        }
                        if let Some(ps) = resp.proof_state {
                            out.push_str(&format!(
                                "\nproofState={} (use this for next lean_tactic call)",
                                ps
                            ));
                        }
                        out
                    }
                }
                None => "lean_tactic unavailable (REPL not ready or sidecar timeout).".into(),
            }
        }
        unknown => format!("Error: unknown tool '{}'", unknown),
    }
}

fn format_results(tool_name: &str, raw: &serde_json::Value) -> String {
    // Check for error
    if let Some(err) = raw.get("error").and_then(|e| e.as_str()) {
        return format!("No results: {}", err);
    }

    let results = match raw.get("results").and_then(|r| r.as_array()) {
        Some(arr) if !arr.is_empty() => arr,
        _ => {
            // For wolfram short format, check "answer" field
            if let Some(answer) = raw.get("answer").and_then(|a| a.as_str()) {
                return format!("[Wolfram] {}", answer);
            }
            return "No results found.".into();
        }
    };

    let mut out = String::new();

    // Tavily includes a direct AI answer at the top level
    if let Some(tavily_answer) = raw.get("tavily_answer").and_then(|a| a.as_str()) {
        if !tavily_answer.is_empty() {
            out.push_str(&format!("[Tavily Answer] {}\n\n", tavily_answer));
        }
    }

    for (i, item) in results.iter().take(5).enumerate() {
        match tool_name {
            "arxiv_search" | "arxiv" => {
                let title = item.get("title").and_then(|t| t.as_str()).unwrap_or("?");
                let authors = item
                    .get("authors")
                    .and_then(|a| a.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|a| a.as_str())
                            .take(3)
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let abs = item.get("abstract").and_then(|a| a.as_str()).unwrap_or("");
                let abs_trunc = if abs.len() > 200 {
                    &abs[..abs.floor_char_boundary(200)]
                } else {
                    abs
                };
                let cats = item
                    .get("categories")
                    .and_then(|c| c.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|c| c.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                out.push_str(&format!(
                    "[arXiv] \"{}\" ({})\n  Authors: {}\n  {}\n\n",
                    title, cats, authors, abs_trunc
                ));
            }
            "oeis_lookup" | "oeis" => {
                let id = item.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                let data = item.get("data").and_then(|d| d.as_str()).unwrap_or("");
                let formula = item
                    .get("formula")
                    .and_then(|f| f.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|f| f.as_str())
                    .unwrap_or("");
                out.push_str(&format!(
                    "[OEIS {}] {}\n  Data: {}\n  Formula: {}\n\n",
                    id, name, data, formula
                ));
            }
            "semantic_scholar_search" | "semantic_scholar" => {
                let title = item.get("title").and_then(|t| t.as_str()).unwrap_or("?");
                let year = item
                    .get("year")
                    .and_then(|y| y.as_i64())
                    .map(|y| y.to_string())
                    .unwrap_or_default();
                let cited = item
                    .get("citationCount")
                    .and_then(|c| c.as_i64())
                    .unwrap_or(0);
                let tldr = item
                    .get("tldr")
                    .and_then(|t| t.as_str())
                    .or_else(|| item.get("abstract").and_then(|a| a.as_str()))
                    .unwrap_or("");
                let tldr_trunc = if tldr.len() > 200 {
                    &tldr[..tldr.floor_char_boundary(200)]
                } else {
                    tldr
                };
                out.push_str(&format!(
                    "[S2] \"{}\" ({}, cited {}x)\n  {}\n\n",
                    title, year, cited, tldr_trunc
                ));
            }
            "tavily_search" | "tavily" => {
                let title = item.get("title").and_then(|t| t.as_str()).unwrap_or("?");
                let url = item.get("url").and_then(|u| u.as_str()).unwrap_or("");
                let content = item
                    .get("abstract")
                    .and_then(|a| a.as_str())
                    .or_else(|| item.get("content").and_then(|c| c.as_str()))
                    .unwrap_or("");
                let content_trunc = if content.len() > 300 {
                    &content[..content.floor_char_boundary(300)]
                } else {
                    content
                };
                out.push_str(&format!(
                    "[Tavily] \"{}\"\n  URL: {}\n  {}\n\n",
                    title, url, content_trunc
                ));
            }
            "loogle_search" | "loogle" => {
                let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("?");
                let ty = item.get("type").and_then(|t| t.as_str()).unwrap_or("?");
                let module = item.get("module").and_then(|m| m.as_str()).unwrap_or("");
                let doc = item.get("doc").and_then(|d| d.as_str()).unwrap_or("");
                let doc_trunc = if doc.len() > 150 {
                    &doc[..doc.floor_char_boundary(150)]
                } else {
                    doc
                };
                out.push_str(&format!(
                    "[Mathlib] {}: {}\n  Module: {}\n  {}\n\n",
                    name, ty, module, doc_trunc
                ));
            }
            _ => {
                // Generic: just dump key fields
                out.push_str(&format!(
                    "Result {}: {}\n\n",
                    i + 1,
                    serde_json::to_string_pretty(item)
                        .unwrap_or_default()
                        .chars()
                        .take(300)
                        .collect::<String>()
                ));
            }
        }
    }

    if out.is_empty() {
        "No results found.".into()
    } else {
        out
    }
}
