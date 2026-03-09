import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AppLoopEvent,
  AppLoopEventName,
  ClaimEventRecord,
  DiagnosticEvent,
  ExtractedPattern,
  KnownLoopEventName,
  LoopEventPayloadMap,
  ReviewFinding,
} from '../types/generated/loopEvents';

const APP_LOOP_EVENT_NAMES = [
  'loop:step_complete',
  'loop:started',
  'loop:attempt_start',
  'loop:retry',
  'loop:outer_complete',
  'loop:finished',
  'loop:audit_complete',
  'loop:step_error',
  'loop:tool_call',
  'loop:claim_check_failed',
  'loop:sympy_correction_attempt',
  'loop:sympy_correction_applied',
  'loop:suspected_answer_disproved',
  'loop:claims_extracted',
  'loop:review_start',
  'loop:review_complete',
  'loop:review_end',
  'loop:sidecar_warmup',
  'loop:obligation_opened',
  'loop:obligation_closed',
  'loop:obligation_assigned',
  'loop:obligation_blocked',
  'loop:satisfaction_signal',
  'loop:pivot_forced',
  'loop:branch_created',
  'loop:branch_closed',
  'loop:branch_switched',
  'loop:fanin_round_start',
  'loop:fanin_round_update',
  'loop:fanin_round_complete',
  'loop:decomposition_start',
  'loop:decomposition_complete',
  'loop:contradiction_detected',
  'loop:evidence_collapse',
  'loop:critic_check',
  'loop:answer_mismatch',
  'loop:claim_conflict',
  'loop:checkpoint_start',
  'loop:patterns_extracted',
  'agent:orchestrator_decision',
  'agent:critic_evaluation',
  'agent:council_finding',
  'agent:scout_result',
  'loop:diagnostic',
  'loop:discerner_finding',
  'loop:error',
  'loop:node_challenged',
  'loop:conclusion_review',
] as const satisfies readonly AppLoopEventName[];

type RecordValue = Record<string, unknown>;

function isRecord(value: unknown): value is RecordValue {
  return typeof value === 'object' && value !== null;
}

function isString(value: unknown): value is string {
  return typeof value === 'string';
}

function isNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function isBoolean(value: unknown): value is boolean {
  return typeof value === 'boolean';
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(isString);
}

function isExtractedPattern(value: unknown): value is ExtractedPattern {
  return isRecord(value)
    && isString(value.name)
    && isString(value.description)
    && isString(value.trigger_text)
    && isString(value.strategy)
    && isString(value.technique_class);
}

function isExtractedPatternArray(value: unknown): value is ExtractedPattern[] {
  return Array.isArray(value) && value.every(isExtractedPattern);
}

function isReviewFinding(value: unknown): value is ReviewFinding {
  return isRecord(value)
    && isString(value.type)
    && isString(value.summary)
    && isString(value.detail)
    && isString(value.priority);
}

function isReviewFindingArray(value: unknown): value is ReviewFinding[] {
  return Array.isArray(value) && value.every(isReviewFinding);
}

function isClaimEventRecord(value: unknown): value is ClaimEventRecord {
  return isRecord(value)
    && isString(value.raw)
    && isString(value.formal)
    && isString(value.source)
    && (value.offset === undefined || isNumber(value.offset));
}

function isClaimEventRecordArray(value: unknown): value is ClaimEventRecord[] {
  return Array.isArray(value) && value.every(isClaimEventRecord);
}

function isDiagnosticEvent(value: unknown): value is DiagnosticEvent {
  return isRecord(value)
    && isString(value.ts)
    && isString(value.category)
    && isString(value.severity)
    && isString(value.source)
    && isString(value.message)
    && (value.step_number === undefined || isNumber(value.step_number))
    && (value.attempt_id === undefined || isString(value.attempt_id));
}

function parseKnownEventPayload<K extends KnownLoopEventName>(
  type: K,
  payload: unknown,
): LoopEventPayloadMap[K] | null {
  const record = isRecord(payload) ? payload : null;

  switch (type) {
    case 'loop:started':
      if (
        record
        && isString(record.event)
        && isString(record.attempt_id)
        && isRecord(record.data)
        && isString(record.data.problem)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:thinking_start':
      if (
        record
        && isString(record.model)
        && (record.step_number === undefined || isNumber(record.step_number))
        && (record.agent_role === undefined || isString(record.agent_role))
        && (record.obligation_id === undefined || isString(record.obligation_id))
        && (record.review === undefined || isBoolean(record.review))
        && (record.manual === undefined || isBoolean(record.manual))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:token':
      if (
        record
        && isString(record.text)
        && (record.agent_role === undefined || isString(record.agent_role))
        && (record.obligation_id === undefined || isString(record.obligation_id))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:thinking_end':
      if (!record) {
        return {} as LoopEventPayloadMap[K];
      }
      if (record.obligation_id === undefined || isString(record.obligation_id)) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:step_complete':
      if (
        record
        && isString(record.attempt_id)
        && isNumber(record.step_number)
        && isString(record.proposal_type)
        && isString(record.proposal_natural)
        && isBoolean(record.verified)
        && isString(record.model)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:finished':
      if (
        !record
        || (
          (record.reason === undefined || isString(record.reason))
          && (record.attempt_id === undefined || isString(record.attempt_id))
        )
      ) {
        return (record ?? {}) as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:review_end':
      return (record ?? {}) as LoopEventPayloadMap[K];

    case 'loop:audit_complete':
      if (
        record
        && isNumber(record.step_number)
        && isNumber(record.breadth)
        && isStringArray(record.techniques_explored)
        && isStringArray(record.techniques_missing)
        && isString(record.recommended_direction)
        && isBoolean(record.should_branch)
        && isNumber(record.confidence)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:step_error':
      if (
        record
        && isNumber(record.step_number)
        && isString(record.error_type)
        && isString(record.model)
        && (record.pattern === undefined || isString(record.pattern))
        && (record.attempt_id === undefined || isString(record.attempt_id))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:tool_call':
      if (
        record
        && isNumber(record.step_number)
        && isString(record.tool)
        && 'input' in record
        && isString(record.result)
        && isNumber(record.result_len)
        && isNumber(record.tool_calls_made)
        && isNumber(record.latency_ms)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:claim_check_failed':
      if (
        record
        && isNumber(record.step_number)
        && isString(record.claim_type)
        && isString(record.reason)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:sympy_correction_attempt':
      if (
        record
        && isNumber(record.step_number)
        && isString(record.original_formal)
        && isString(record.diff)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:sympy_correction_applied':
      if (
        record
        && isNumber(record.step_number)
        && isString(record.original)
        && isString(record.corrected)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:suspected_answer_disproved':
      if (
        record
        && isNumber(record.step_number)
        && isString(record.suspected_value)
        && isString(record.source)
        && isString(record.reason)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:claims_extracted':
      if (
        record
        && isNumber(record.step_number)
        && isClaimEventRecordArray(record.claims)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:review_start':
      if (
        record
        && (record.attempt_id === undefined || isString(record.attempt_id))
        && (record.manual === undefined || isBoolean(record.manual))
        && (record.problem_id === undefined || isString(record.problem_id))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:review_complete':
      if (
        record
        && isString(record.attempt_id)
        && isNumber(record.findings_count)
        && isNumber(record.exploration_coverage)
        && isBoolean(record.conclusion_sound)
        && isNumber(record.conclusion_confidence)
        && isString(record.training_label)
        && isStringArray(record.missing_constructions)
        && isReviewFindingArray(record.findings)
        && (record.manual === undefined || isBoolean(record.manual))
        && (record.problem_id === undefined || isString(record.problem_id))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:attempt_start':
      if (
        record
        && isString(record.attempt_id)
        && isNumber(record.attempt_number)
        && isNumber(record.max_attempts)
        && (record.constraints === undefined || isStringArray(record.constraints))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:retry':
      if (
        record
        && isNumber(record.attempt_number)
        && isNumber(record.max_attempts)
        && isString(record.new_attempt_id)
        && (record.previous_attempt_id === undefined || isString(record.previous_attempt_id))
        && (record.reason === undefined || isString(record.reason))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:outer_complete':
      if (
        record
        && isNumber(record.attempts_used)
        && isString(record.final_attempt_id)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:sidecar_warmup':
      if (
        record
        && isString(record.status)
        && ['waiting', 'warming', 'ready', 'timeout'].includes(record.status)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:obligation_opened':
      if (
        record
        && isString(record.id)
        && isString(record.description)
        && isString(record.obligation_type)
        && isNumber(record.priority)
        && (record.decomposition_id === undefined || isString(record.decomposition_id))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:obligation_closed':
      if (
        record
        && isString(record.id)
        && isString(record.status)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:obligation_assigned':
      if (
        record
        && isString(record.obligation_id)
        && isString(record.assigned_model)
        && isNumber(record.steps_spent)
        && isNumber(record.max_steps)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:obligation_blocked':
      if (
        record
        && isString(record.description)
        && isString(record.reason)
        && isString(record.audit_session_id)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:pivot_forced':
      if (
        record
        && isString(record.obligation_desc)
        && isString(record.blacklisted_technique)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:branch_created':
      if (
        record
        && isNumber(record.branch_id)
        && isNumber(record.parent_branch)
        && isString(record.fork_reason)
        && isString(record.direction)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:branch_closed':
      if (
        record
        && isNumber(record.branch_id)
        && isString(record.status)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:branch_switched':
      if (
        record
        && isNumber(record.from_branch)
        && isNumber(record.to_branch)
        && isString(record.reason)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:decomposition_start':
      if (
        record
        && isString(record.attempt_id)
        && isString(record.trigger)
        && isString(record.model)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:decomposition_complete':
      if (
        record
        && isString(record.attempt_id)
        && isString(record.decomposition_id)
        && isNumber(record.obligations_created)
        && isString(record.problem_type)
        && isString(record.model)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:contradiction_detected':
      if (
        record
        && isString(record.obligation_id)
        && isString(record.description)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:evidence_collapse':
      if (
        record
        && isNumber(record.step_number)
        && isNumber(record.collapsed_count)
        && isNumber(record.evidence_bound)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:obligation_gate':
      if (
        record
        && isNumber(record.step_number)
        && isNumber(record.open_count)
        && isStringArray(record.obligations)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:answer_mismatch':
      if (
        record
        && isNumber(record.step_number)
        && isString(record.proposed_answer)
        && isString(record.known_answer)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:claim_conflict':
      if (
        record
        && isNumber(record.step_number)
        && isString(record.conflict)
        && isClaimEventRecordArray(record.claims)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:checkpoint_start':
      if (
        record
        && isNumber(record.step_number)
        && isNumber(record.open_obligations)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:critic_check':
      if (
        record
        && isString(record.obligation_id)
        && isString(record.check_description)
        && isString(record.expected_if_correct)
        && isString(record.counterexample_hint)
        && isBoolean(record.likely_wrong)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:patterns_extracted':
      if (
        record
        && isString(record.attempt_id)
        && isNumber(record.count)
        && isExtractedPatternArray(record.patterns)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'agent:orchestrator_decision':
      if (record && isString(record.type)) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'agent:critic_evaluation':
      if (
        record
        && isString(record.obligation_id)
        && isString(record.check_description)
        && isBoolean(record.likely_wrong)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'agent:council_finding':
      if (
        record
        && isString(record.obligation_id)
        && isNumber(record.tally_yes)
        && isNumber(record.tally_total)
        && isString(record.outcome)
        && (record.note === undefined || isString(record.note))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'agent:scout_result':
      if (
        record
        && isString(record.trigger)
        && (record.trigger === 'pre_solve' || record.trigger === 'mid_solve')
        && isNumber(record.results_count)
        && isStringArray(record.sources)
        && isString(record.briefing)
        && (record.obligation_id === undefined || isString(record.obligation_id))
        && (record.obligation_desc === undefined || isString(record.obligation_desc))
        && (record.blacklisted_techniques === undefined || isStringArray(record.blacklisted_techniques))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:diagnostic':
      return isDiagnosticEvent(record) ? (record as LoopEventPayloadMap[K]) : null;

    case 'loop:discerner_finding':
      if (
        record
        && isString(record.attempt_id)
        && isNumber(record.step_number)
        && isNumber(record.failure_streak)
        && isString(record.classification)
        && isString(record.root_cause)
        && isString(record.recommendation)
        && isNumber(record.confidence)
        && isString(record.suggested_action)
        && isString(record.discerner_model)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:error':
      if (record && isString(record.message)) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:satisfaction_signal':
      if (
        record
        && isString(record.obligation_id)
        && isString(record.source)
        && isBoolean(record.satisfies)
        && isNumber(record.tally_yes)
        && isNumber(record.tally_total)
        && (record.note === undefined || isString(record.note))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:solver_self_assessment':
      if (
        record
        && isNumber(record.step_number)
        && isString(record.targets_obligation)
        && isBoolean(record.closes_obligation)
        && isString(record.closure_reason)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:node_challenged':
      if (
        record
        && isNumber(record.step_number)
        && isString(record.adversary_model)
        && isBoolean(record.flaw_found)
        && isString(record.attack_vector)
        && isNumber(record.confidence)
        && isBoolean(record.fatal)
        && (record.suggested_fix === undefined || isString(record.suggested_fix))
        && (record.reasoning === undefined || isString(record.reasoning))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:conclusion_review':
      if (
        record
        && isNumber(record.step_number)
        && isBoolean(record.sound)
        && isNumber(record.confidence)
        && isString(record.reason)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:fanin_round_start':
      if (
        record
        && isString(record.attempt_id)
        && isString(record.solver_round_id)
        && isString(record.obligation_id)
        && isString(record.obligation_desc)
        && isNumber(record.worker_count)
        && isStringArray(record.worker_models)
        && Array.isArray(record.reserved_step_numbers)
        && record.reserved_step_numbers.every(isNumber)
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:fanin_round_update':
      if (
        record
        && isString(record.solver_round_id)
        && (record.status === undefined || isString(record.status))
        && (record.action === undefined || isString(record.action))
        && (record.reason === undefined || isString(record.reason))
        && (record.worker_id === undefined || isString(record.worker_id))
        && (record.step_number === undefined || isNumber(record.step_number))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    case 'loop:fanin_round_complete':
      if (
        record
        && isString(record.solver_round_id)
        && isNumber(record.results_processed)
        && (record.workers_dispatched === undefined || isNumber(record.workers_dispatched))
        && (record.results_skipped_stale === undefined || isNumber(record.results_skipped_stale))
        && (record.early_exit === undefined || isBoolean(record.early_exit))
      ) {
        return record as LoopEventPayloadMap[K];
      }
      return null;

    default:
      return null;
  }
}

function warnInvalidPayload(type: string, payload: unknown) {
  console.warn(`[events] ignored invalid payload for ${type}`, payload);
}

export function listenKnownEvent<K extends KnownLoopEventName>(
  type: K,
  callback: (payload: LoopEventPayloadMap[K]) => void,
): Promise<UnlistenFn> {
  return listen(type, (event) => {
    const parsed = parseKnownEventPayload(type, event.payload);
    if (parsed) {
      callback(parsed);
    } else {
      warnInvalidPayload(type, event.payload);
    }
  });
}

export function onLoopEvent(callback: (event: AppLoopEvent) => void): () => void {
  let cancelled = false;
  const settled: UnlistenFn[] = [];

  for (const name of APP_LOOP_EVENT_NAMES) {
    listenKnownEvent(name, (payload) => {
      if (!cancelled) {
        callback({ type: name, payload } as AppLoopEvent);
      }
    }).then((unlisten) => {
      if (cancelled) {
        unlisten();
      } else {
        settled.push(unlisten);
      }
    });
  }

  return () => {
    cancelled = true;
    settled.forEach((unlisten) => unlisten());
  };
}
