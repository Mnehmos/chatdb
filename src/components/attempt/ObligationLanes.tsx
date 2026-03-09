import type { Step, ObligationEvent } from '../../types';
import { MiniTokenStream } from '../loop/MiniTokenStream';

interface Props {
  steps: Step[];
  obligations: ObligationEvent[];
  status: string;
  onStepClick?: (stepId: string) => void;
  focusedObligationId?: string | null;
  onObligationFocus?: (obligationId: string | null) => void;
}

const priorityLabel = (p: number): string => {
  if (p >= 0.9) return 'S+';
  if (p >= 0.7) return 'S';
  if (p >= 0.5) return 'A';
  if (p >= 0.3) return 'B';
  return 'C';
};

const hasClosingMajority = (yes?: number, total?: number): boolean =>
  yes != null && total != null && total >= 3 && yes * 2 > total;

interface LaneProps {
  obligation: ObligationEvent | null;
  steps: Step[];
  isRunning: boolean;
  onStepClick?: (stepId: string) => void;
  focused?: boolean;
  onFocus?: () => void;
}

function ObLane({ obligation, steps, isRunning, onStepClick, focused, onFocus }: LaneProps) {
  const laneSteps = obligation
    ? steps.filter(s => s.obligation_id === obligation.id)
    : steps.filter(s => !s.obligation_id);

  const isOpen = !obligation || obligation.status === 'open' || obligation.status === 'assigned';
  const isSolved = obligation?.status === 'closed_proved';
  const isFailed = obligation?.status === 'closed_refuted';

  const headerMod = isSolved ? 'ob-lane-header--solved'
    : isFailed ? 'ob-lane-header--failed'
    : 'ob-lane-header--open';

  const isAssigned = obligation?.status === 'assigned';
  const statusLabel = isSolved ? 'SOLVED'
    : isFailed ? 'FAILED'
    : isAssigned ? 'ACTIVE'
    : 'OPEN';

  return (
    <div className={`obligation-lane${isSolved ? ' lane-solved' : isFailed ? ' lane-failed' : ''}${focused ? ' lane-focused' : ''}`}>
      <div className={`ob-lane-header ${headerMod}`}>
        {obligation ? (
          <>
            <div className="ob-lane-top">
              <span className="ob-lane-priority">[{priorityLabel(obligation.priority)}]</span>
              <span className="ob-lane-type">{obligation.obligation_type}</span>
              <span className="ob-lane-status-badge">{statusLabel}</span>
            </div>
            <div className="ob-lane-desc" title={obligation.description}>
              {obligation.description.length > 72
                ? obligation.description.slice(0, 72) + '…'
                : obligation.description}
            </div>
            {obligation.tally_yes != null && obligation.tally_total != null && obligation.tally_total > 0 && (
              <div className="ob-lane-votes">
                <span className={`ob-lane-vote-count ${hasClosingMajority(obligation.tally_yes, obligation.tally_total) ? 'votes-pass' : 'votes-fail'}`}>
                  {obligation.tally_yes}/{obligation.tally_total} votes
                </span>
              </div>
            )}
          </>
        ) : (
          <div className="ob-lane-top">
            <span className="ob-lane-type">GENERAL</span>
            <span className="ob-lane-status-badge" style={{ color: 'var(--text-muted)' }}>unattributed</span>
          </div>
        )}
      </div>

      {/* Mini token stream — live solver output for this obligation */}
      {isRunning && obligation && isOpen && (
        <MiniTokenStream
          obligationId={obligation.id}
          focused={focused}
          onFocus={onFocus}
        />
      )}

      <div className="ob-lane-steps">
        {laneSteps.map(step => (
          <div
            key={step.id}
            className={`ob-step-card ${step.verified ? 'ob-step-ok' : 'ob-step-fail'}`}
            onClick={() => onStepClick?.(step.id)}
            style={{ cursor: onStepClick ? 'pointer' : undefined }}
          >
            <div className="ob-step-meta">
              <span className="ob-step-num">#{step.step_number}</span>
              {step.proposal_type && step.proposal_type !== 'algebraic' && (
                <span className="ob-step-type">{step.proposal_type.toUpperCase()}</span>
              )}
              <span className="ob-step-verdict">{step.verified ? '✓' : '✗'}</span>
              {step.challenge_confidence != null && (
                <span className={`ob-step-challenge ${step.challenge_fatal ? 'ch-fatal' : 'ch-ok'}`}>
                  {Math.round(step.challenge_confidence * 100)}%
                </span>
              )}
            </div>
            <div className="ob-step-text">{step.proposal_natural}</div>
            {!step.verified && step.rejection_reason && (
              <div className="ob-step-rejection">{step.rejection_reason.slice(0, 80)}</div>
            )}
          </div>
        ))}

        {/* Pulsing waiting indicator while running and no steps yet */}
        {isRunning && laneSteps.length === 0 && isOpen && (
          <div className="ob-lane-waiting">
            <span className="waiting-ellipsis">···</span>
          </div>
        )}

        {/* Empty non-running lane */}
        {!isRunning && laneSteps.length === 0 && (
          <div className="ob-lane-empty">no steps</div>
        )}
      </div>
    </div>
  );
}

export function ObligationLanes({ steps, obligations, status, onStepClick, focusedObligationId, onObligationFocus }: Props) {
  const isRunning = status === 'running';

  // Sort: assigned first, then open, then closed — by priority within each group
  const statusOrder = (s: string) => {
    if (s === 'assigned') return 0;
    if (s === 'open') return 1;
    if (s === 'closed_proved') return 2;
    if (s === 'closed_refuted') return 3;
    return 4;
  };
  const sorted = [...obligations].sort((a, b) => {
    const sd = statusOrder(a.status) - statusOrder(b.status);
    if (sd !== 0) return sd;
    return b.priority - a.priority;
  });

  // Show all obligations — don't cap at parallel count since we need to see closed ones too
  const visible = sorted;

  // Show general lane only if there are actually unattributed steps
  const hasGeneral = steps.some(s => !s.obligation_id);

  return (
    <div className="obligation-lanes">
      {hasGeneral && (
        <ObLane obligation={null} steps={steps} isRunning={isRunning} onStepClick={onStepClick} />
      )}

      {visible.map(ob => (
        <ObLane
          key={ob.id}
          obligation={ob}
          steps={steps}
          isRunning={isRunning}
          onStepClick={onStepClick}
          focused={focusedObligationId === ob.id}
          onFocus={() => onObligationFocus?.(
            focusedObligationId === ob.id ? null : ob.id
          )}
        />
      ))}

      {/* All obligations shown */}
    </div>
  );
}
