use crate::api::sidecar::{SidecarClient, ValidateStepRequest};
use crate::db::{Database, StepRecord};

pub struct VerificationPipeline {
    sidecar: SidecarClient,
}

/// Rich result returned from validation + recording.
pub struct StepResult {
    pub step_id: String,
    pub node_id: String,
    pub verified: bool,
    pub sympy_passed: Option<bool>,
    pub pint_passed: Option<bool>,
    pub lean_passed: Option<bool>,
    pub rejection_reason: Option<String>,
    pub wall_time_ms: u32,
}

/// Extra context the loop engine passes in alongside the proposal.
pub struct StepContext<'a> {
    pub attempt_id: &'a str,
    pub parent_step_id: Option<&'a str>,
    pub step_number: u32,
    pub model: &'a str,
    pub goal_state: &'a str,
    pub context_refs: Option<&'a str>,
    pub context_provided: Option<&'a str>,
    pub proposal_reasoning: Option<&'a str>,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    /// RED-012: When this step targets a specific obligation, its ID goes here.
    pub obligation_ref: Option<&'a str>,
    /// Branch this step belongs to. Propagated to proof_nodes and obligations.
    pub branch_id: i32,
    /// Problem domain (e.g., "algebra", "number_theory") for verifier selection.
    pub problem_domain: Option<&'a str>,
}

impl Default for VerificationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl VerificationPipeline {
    pub fn new() -> Self {
        Self {
            sidecar: SidecarClient::new(),
        }
    }

    pub async fn validate_and_record(
        &self,
        db: &Database,
        ctx: &StepContext<'_>,
        proposal_type: &str,
        proposal_natural: &str,
        proposal_formal: Option<&str>,
        proposal_formal_lean: Option<&str>,
        run_lean: bool,
        proposal_claim: Option<&serde_json::Value>,
    ) -> Result<StepResult, String> {
        let resp = self
            .sidecar
            .validate_step(&ValidateStepRequest {
                proposal_type: proposal_type.to_string(),
                proposal_natural: proposal_natural.to_string(),
                proposal_formal: proposal_formal.map(|s| s.to_string()),
                proposal_formal_lean: proposal_formal_lean.map(|s| s.to_string()),
                goal_state: ctx.goal_state.to_string(),
                run_lean,
                problem_domain: ctx.problem_domain.map(|s| s.to_string()),
                proposal_claim: proposal_claim.cloned(),
            })
            .await
            .map_err(|e| format!("Sidecar: {}", e))?;

        // Build rejection reason from individual validator messages
        let rejection = if !resp.all_passed {
            let mut r = vec![];
            if let Some(ref s) = resp.sympy {
                if !s.passed {
                    r.push(format!("SymPy: {}", s.message));
                }
            }
            if let Some(ref p) = resp.pint {
                if !p.passed {
                    r.push(format!("Pint: {}", p.message));
                }
            }
            if let Some(ref l) = resp.lean {
                if !l.passed {
                    if !l.raw_output.is_empty() {
                        r.push(format!("Lean: {} [code: {}]", l.message, l.raw_output));
                    } else {
                        r.push(format!("Lean: {}", l.message));
                    }
                }
            }
            Some(r.join("; "))
        } else {
            None
        };

        // Collect detailed validator output strings for DB
        let sympy_result = resp.sympy.as_ref().map(|s| {
            if s.raw_output.is_empty() {
                s.message.clone()
            } else {
                format!("{}: {}", s.message, s.raw_output)
            }
        });
        let pint_result = resp.pint.as_ref().map(|p| {
            if p.raw_output.is_empty() {
                p.message.clone()
            } else {
                format!("{}: {}", p.message, p.raw_output)
            }
        });
        let lean_result = resp.lean.as_ref().map(|l| {
            if l.raw_output.is_empty() {
                l.message.clone()
            } else {
                format!("{}: {}", l.message, l.raw_output)
            }
        });

        // Sum wall times from all validators
        let total_wall_ms: u32 = [&resp.sympy, &resp.pint, &resp.lean]
            .iter()
            .filter_map(|v| v.as_ref())
            .map(|v| v.wall_time_ms)
            .sum();

        // Record everything to DB
        let step_id = db
            .record_step(&StepRecord {
                attempt_id: ctx.attempt_id,
                parent_step_id: ctx.parent_step_id,
                step_number: ctx.step_number,
                model: ctx.model,
                context_refs: ctx.context_refs,
                goal_state: ctx.goal_state,
                context_provided: ctx.context_provided,
                proposal_type,
                proposal_natural,
                proposal_formal,
                proposal_reasoning: ctx.proposal_reasoning,
                sympy_result: sympy_result.as_deref(),
                sympy_passed: resp.sympy.as_ref().map(|s| s.passed),
                pint_result: pint_result.as_deref(),
                pint_passed: resp.pint.as_ref().map(|p| p.passed),
                lean_result: lean_result.as_deref(),
                lean_passed: resp.lean.as_ref().map(|l| l.passed),
                verified: resp.all_passed,
                rejection_reason: rejection.as_deref(),
                model_tokens_in: ctx.tokens_in,
                model_tokens_out: ctx.tokens_out,
                wall_time_ms: Some(total_wall_ms),
                challenge_model: None,
                challenge_flaw_found: None,
                challenge_attack: None,
                challenge_confidence: None,
                challenge_fatal: None,
                obligation_id: ctx.obligation_ref,
                solver_round_id: None,
                solver_worker_id: None,
                solver_dispatch_mode: None,
                stale_sibling: false,
            })
            .map_err(|e| e.to_string())?;

        // Dual-write: also create a proof_node linked to this step.
        // Resolve real parent node ID from the parent step's proof_node.
        let parent_ids = if let Some(parent_sid) = ctx.parent_step_id {
            match db.get_node_by_step_id(parent_sid) {
                Ok(Some(parent_node)) => Some(parent_node.id),
                _ => {
                    tracing::debug!("No parent proof_node found for step_id={}", parent_sid);
                    None
                }
            }
        } else {
            None
        };

        let node_type = match proposal_type {
            "conclusion" => "closure",
            "algebraic" | "claim" => "claim",
            "construction" => "construction",
            "bound" => "bound",
            "case_split" => "case_split",
            "reduction" => "reduction",
            _ => "claim",
        };
        let node_status = if resp.all_passed {
            "verified"
        } else {
            "rejected"
        };
        let validator_used = {
            let mut v = vec![];
            if resp.sympy.is_some() {
                v.push("sympy");
            }
            if resp.pint.is_some() {
                v.push("pint");
            }
            if resp.lean.is_some() {
                v.push("lean");
            }
            v.join(",")
        };
        let validator_result_json = serde_json::json!({
            "all_passed": resp.all_passed,
            "sympy_passed": resp.sympy.as_ref().map(|s| s.passed),
            "pint_passed": resp.pint.as_ref().map(|p| p.passed),
            "lean_passed": resp.lean.as_ref().map(|l| l.passed),
        })
        .to_string();

        // RED-011: Populate technique_class from proposal_type (Layer 0 — basic mapping).
        // Layer 1 self-tagging will override this with finer-grained classification in Sprint 2.
        let technique_class = match proposal_type {
            "conclusion" => None, // closures don't have a technique
            _ => Some(proposal_type),
        };

        let node_id = match db.create_node(
            ctx.attempt_id,
            ctx.branch_id,
            node_type,
            parent_ids.as_deref(),
            proposal_natural,
            proposal_formal,
            technique_class,
            None, // construction_family — Layer 1 self-tagging in Sprint 2
            node_status,
            Some(&validator_used),
            Some(&validator_result_json),
            Some(ctx.model),
            ctx.obligation_ref,
            Some(&step_id),
            ctx.tokens_in,
            ctx.step_number,
        ) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!(error = %e, step_id = %step_id, "Failed to create proof_node — DAG will be incomplete");
                String::new()
            }
        };

        // Append dag_event for this node creation
        let event_payload = serde_json::json!({
            "node_id": &node_id,
            "step_id": &step_id,
            "node_type": node_type,
            "status": node_status,
            "parent_ids": &parent_ids,
        })
        .to_string();
        let _ = db.append_dag_event(
            ctx.attempt_id,
            if resp.all_passed {
                "node_verified"
            } else {
                "node_rejected"
            },
            &event_payload,
            "verification_pipeline",
        );

        tracing::info!(
            step_id = %step_id,
            step = ctx.step_number,
            verified = resp.all_passed,
            sympy = ?resp.sympy.as_ref().map(|s| s.passed),
            pint = ?resp.pint.as_ref().map(|p| p.passed),
            lean = ?resp.lean.as_ref().map(|l| l.passed),
            wall_ms = total_wall_ms,
            tokens_in = ?ctx.tokens_in,
            tokens_out = ?ctx.tokens_out,
            "Step validated and recorded"
        );

        Ok(StepResult {
            step_id,
            node_id,
            verified: resp.all_passed,
            sympy_passed: resp.sympy.as_ref().map(|s| s.passed),
            pint_passed: resp.pint.as_ref().map(|p| p.passed),
            lean_passed: resp.lean.as_ref().map(|l| l.passed),
            rejection_reason: rejection,
            wall_time_ms: total_wall_ms,
        })
    }
}
