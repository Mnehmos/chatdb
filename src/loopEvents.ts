import { useAgentStore } from './stores/agentStore';
import { useDiagnosticStore } from './stores/diagnosticStore';
import { useLoopStore } from './stores/loopStore';
import { useNavigationStore } from './stores/navigationStore';
import { useProblemStore } from './stores/problemStore';
import { useWorkspaceStore } from './stores/workspaceStore';
import type {
  AppLoopEvent,
  DiagnosticEvent,
  LoopSidecarWarmupPayload,
  LoopStepCompletePayload,
  SatisfactionSignalEvent,
} from './types/generated/loopEvents';
import type { Step as StepRecord } from './types';

type DiagnosticEventInput = Omit<DiagnosticEvent, 'ts'>;

export function buildStepFromPayload(payload: LoopStepCompletePayload): StepRecord {
  return {
    id: `step-${Date.now()}-${payload.step_number}`,
    attempt_id: payload.attempt_id,
    step_number: payload.step_number,
    model: payload.model,
    goal_state: '',
    proposal_type: payload.proposal_type,
    proposal_natural: payload.proposal_natural,
    proposal_formal: payload.proposal_formal,
    proposal_reasoning: payload.proposal_reasoning,
    verified: payload.verified,
    rejection_reason: payload.rejection_reason,
    sympy_passed: payload.sympy_passed ?? undefined,
    pint_passed: payload.pint_passed ?? undefined,
    lean_passed: payload.lean_passed ?? undefined,
    challenge_model: payload.challenge_model,
    challenge_flaw_found: payload.challenge_flaw_found,
    challenge_attack: payload.challenge_attack,
    challenge_confidence: payload.challenge_confidence,
    challenge_fatal: payload.challenge_fatal,
    obligation_id: payload.obligation_id,
    obligation_desc: payload.obligation_desc,
    obligation_type: payload.obligation_type,
    solver_round_id: payload.solver_round_id,
    solver_worker_id: payload.solver_worker_id,
    solver_dispatch_mode: payload.solver_dispatch_mode,
    stale_sibling: payload.stale_sibling,
    created_at: new Date().toISOString(),
  };
}

function getActiveProblemId(): string | null {
  return useNavigationStore.getState().problemId;
}

function refreshAttempts(): string | null {
  const problemId = getActiveProblemId();
  if (problemId) {
    useWorkspaceStore.getState().loadAttempts(problemId);
  }
  return problemId;
}

function addDiagnosticEvent(event: DiagnosticEventInput) {
  useDiagnosticStore.getState().addEvent({
    ts: new Date().toISOString(),
    ...event,
  });
}

function setWarmupStatus(status: LoopSidecarWarmupPayload['status']) {
  const loopStore = useLoopStore.getState();
  if (status === 'waiting' || status === 'warming') {
    loopStore.setWarmupStatus('waiting');
  } else if (status === 'ready') {
    loopStore.setWarmupStatus('ready');
  } else if (status === 'timeout') {
    loopStore.setWarmupStatus('timeout');
  }
}

function handleSatisfactionSignal(payload: SatisfactionSignalEvent) {
  const loopStore = useLoopStore.getState();
  const existing = loopStore.obligations.find((obligation) => obligation.id === payload.obligation_id);
  const prevSignals = payload.tally_total === 1 ? [] : (existing?.signals ?? []);

  loopStore.updateObligation(payload.obligation_id, {
    tally_yes: payload.tally_yes,
    tally_total: payload.tally_total,
    signals: [
      ...prevSignals,
      {
        source: payload.source,
        satisfies: payload.satisfies,
        note: payload.note,
      },
    ],
  });
}

export function handleLoopEvent(event: AppLoopEvent) {
  const loopStore = useLoopStore.getState();

  switch (event.type) {
    case 'loop:step_complete':
      loopStore.addStep(buildStepFromPayload(event.payload));
      return;

    case 'loop:started':
      loopStore.setStatus('running');
      return;

    case 'loop:attempt_start':
      loopStore.setAttemptInfo(
        event.payload.attempt_number,
        event.payload.max_attempts,
        event.payload.attempt_id,
      );
      refreshAttempts();
      return;

    case 'loop:retry':
      loopStore.resetForNewAttempt(
        event.payload.new_attempt_id,
        event.payload.attempt_number,
      );
      refreshAttempts();
      return;

    case 'loop:outer_complete': {
      loopStore.setStatus('finished');
      useProblemStore.getState().fetchProblems();
      const problemId = refreshAttempts();
      if (problemId) {
        useWorkspaceStore.getState().loadReports(problemId);
      }
      return;
    }

    case 'loop:finished':
      return;

    case 'loop:audit_complete':
      loopStore.addAudit(event.payload);
      return;

    case 'loop:step_error':
    case 'loop:tool_call':
    case 'loop:claim_check_failed':
    case 'loop:sympy_correction_attempt':
    case 'loop:sympy_correction_applied':
    case 'loop:suspected_answer_disproved':
    case 'loop:claims_extracted':
      return;

    case 'loop:review_start':
      loopStore.setReviewing(true);
      return;

    case 'loop:review_complete':
      if (!loopStore.reviewResult?.manual || event.payload.manual) {
        loopStore.setReview(event.payload);
      }
      return;

    case 'loop:review_end':
      loopStore.setReviewing(false);
      return;

    case 'loop:sidecar_warmup':
      setWarmupStatus(event.payload.status);
      return;

    case 'loop:obligation_opened':
      loopStore.addObligation({
        id: event.payload.id,
        description: event.payload.description,
        obligation_type: event.payload.obligation_type,
        priority: event.payload.priority,
        status: 'open',
      });
      return;

    case 'loop:obligation_closed':
      loopStore.updateObligation(event.payload.id, {
        status: event.payload.status,
        closed_by_step: event.payload.closed_by_step,
        closure_note: event.payload.closure_note,
      });
      return;

    case 'loop:obligation_assigned':
      loopStore.updateObligation(event.payload.obligation_id, {
        status: 'assigned',
        assigned_model: event.payload.assigned_model,
        steps_spent: event.payload.steps_spent,
        max_steps: event.payload.max_steps,
      });
      return;

    case 'loop:obligation_blocked':
      return;

    case 'loop:satisfaction_signal':
      handleSatisfactionSignal(event.payload);
      return;

    case 'loop:pivot_forced':
      console.log(
        '[pivot_forced]',
        event.payload.obligation_desc,
        '- blacklisted:',
        event.payload.blacklisted_technique,
      );
      return;

    case 'loop:branch_created':
    case 'loop:branch_closed':
    case 'loop:branch_switched':
      return;

    case 'loop:fanin_round_start':
      console.log(
        '[fanin_start]',
        event.payload.obligation_id,
        '- workers:',
        event.payload.worker_count,
        'models:',
        event.payload.worker_models,
      );
      loopStore.updateObligation(event.payload.obligation_id, {
        dispatch_mode: 'parallel_fanin',
        assigned_models: event.payload.worker_models,
        active_solver_round_id: event.payload.solver_round_id,
      });
      useAgentStore.getState().addOrchestratorEvent({
        type: 'fanin_start',
        obligation_id: event.payload.obligation_id,
        worker_count: event.payload.worker_count,
        worker_models: event.payload.worker_models,
      });
      return;

    case 'loop:fanin_round_update':
      console.log(
        '[fanin_update]',
        event.payload.solver_round_id,
        event.payload.action,
        event.payload.reason,
      );
      return;

    case 'loop:fanin_round_complete':
      console.log(
        '[fanin_complete]',
        event.payload.solver_round_id,
        '- processed:',
        event.payload.results_processed,
        'stale:',
        event.payload.results_skipped_stale,
      );
      useAgentStore.getState().addOrchestratorEvent({
        type: 'fanin_complete',
        results_processed: event.payload.results_processed,
      });
      return;

    case 'loop:decomposition_start':
      console.log('[decomposition]', event.payload.trigger, 'model:', event.payload.model);
      return;

    case 'loop:decomposition_complete':
      console.log(
        '[decomposition_complete]',
        event.payload.obligations_created,
        'obligations created',
      );
      return;

    case 'loop:contradiction_detected':
      console.warn(
        '[CONTRADICTION]',
        event.payload.description,
        '- obligation:',
        event.payload.obligation_id,
      );
      loopStore.addObligation({
        id: event.payload.obligation_id,
        description: `RESOLVE: ${event.payload.description}`,
        obligation_type: 'RESOLVE',
        priority: 1.0,
        status: 'open',
      });
      return;

    case 'loop:evidence_collapse':
      return;

    case 'loop:critic_check':
      loopStore.addCriticCheck(event.payload);
      return;

    case 'loop:answer_mismatch':
    case 'loop:claim_conflict':
    case 'loop:checkpoint_start':
      return;

    case 'loop:patterns_extracted':
      loopStore.setExtractedPatterns(event.payload.patterns);
      return;

    case 'agent:orchestrator_decision':
      useAgentStore.getState().addOrchestratorEvent(event.payload);
      return;

    case 'agent:critic_evaluation':
      useAgentStore.getState().addCriticEvent(event.payload);
      return;

    case 'agent:council_finding':
      useAgentStore.getState().addCouncilSession(event.payload);
      return;

    case 'agent:scout_result':
      useAgentStore.getState().addScoutResult(event.payload);
      return;

    case 'loop:node_challenged':
      addDiagnosticEvent({
        category: 'info',
        severity: event.payload.fatal ? 'warn' : 'info',
        source: 'challenger',
        step_number: event.payload.step_number,
        message: `Challenge: ${event.payload.flaw_found ? 'flaw found' : 'no flaw'} (${Math.round(event.payload.confidence * 100)}%)`,
        detail: event.payload.attack_vector,
      });
      return;

    case 'loop:conclusion_review':
      addDiagnosticEvent({
        category: 'gate',
        severity: event.payload.sound ? 'info' : 'warn',
        source: 'reviewer',
        step_number: event.payload.step_number,
        message: `Conclusion: ${event.payload.sound ? 'SOUND' : 'UNSOUND'} (${Math.round(event.payload.confidence * 100)}%)`,
        detail: event.payload.reason,
      });
      return;

    case 'loop:diagnostic':
      useDiagnosticStore.getState().addEvent(event.payload);
      return;

    case 'loop:discerner_finding':
      loopStore.addDiscernerFinding(event.payload);
      return;

    case 'loop:error':
      addDiagnosticEvent({
        category: 'mechanical',
        severity: 'fatal',
        source: 'loop_engine',
        message: event.payload.message,
      });
      useDiagnosticStore.getState().setOpen(true);
      loopStore.setStatus('idle');
      return;

    default: {
      const exhaustive: never = event;
      return exhaustive;
    }
  }
}
