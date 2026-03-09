import { useEffect, useRef, useState } from 'react';
import { listenKnownEvent } from '../../services/events';

const ROLE_LABELS: Record<string, string> = {
  solver: 'SOLVER',
  adversary: 'ADVERSARY',
  reviewer: 'REVIEWER',
  auditor: 'AUDITOR',
  critic: 'CRITIC',
  discerner: 'DISCERNER',
  challenger: 'CHALLENGER',
  checkpoint_reviewer: 'REVIEW',
  checkpoint_adversary: 'ADVERSARY',
};

interface Props {
  focusObligationId?: string | null;
}

export function ThinkingPanel({ focusObligationId }: Props = {}) {
  const [text, setText] = useState('');
  const [active, setActive] = useState(false);
  const [stepInfo, setStepInfo] = useState<{ step_number?: number; model: string } | null>(null);
  const [agentRole, setAgentRole] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState(false);
  const bodyRef = useRef<HTMLPreElement>(null);
  const stepInfoRef = useRef<{ step_number?: number; model: string } | null>(null);
  const focusRef = useRef<string | null | undefined>(focusObligationId);

  useEffect(() => {
    focusRef.current = focusObligationId;
  }, [focusObligationId]);

  useEffect(() => {
    const promises: Promise<() => void>[] = [];
    const shouldDisplayStream = (obligationId?: string) => {
      const focus = focusRef.current;
      if (focus) {
        return obligationId === focus;
      }
      return obligationId == null;
    };

    promises.push(
      listenKnownEvent('loop:thinking_start', (payload) => {
        if (!shouldDisplayStream(payload.obligation_id)) {
          return;
        }

        const newRole = payload.agent_role || 'solver';
        const roleLabel = ROLE_LABELS[newRole] || newRole.toUpperCase();
        const prev = stepInfoRef.current;

        if (!prev || prev.step_number !== payload.step_number) {
          setText(`--- ${roleLabel} ---\n`);
        } else {
          setText((current) => `${current}\n\n--- ${roleLabel} ---\n`);
        }
        setActive(true);
        setStepInfo({ step_number: payload.step_number, model: payload.model });
        stepInfoRef.current = { step_number: payload.step_number, model: payload.model };
        setAgentRole(newRole);
      }),
    );

    promises.push(
      listenKnownEvent('loop:token', (payload) => {
        if (!shouldDisplayStream(payload.obligation_id)) {
          return;
        }

        setText((current) => current + payload.text);
        if (payload.agent_role) {
          const nextRole = payload.agent_role;
          setAgentRole((current) => (current !== nextRole ? nextRole : current));
        }
      }),
    );

    promises.push(
      listenKnownEvent('loop:thinking_end', (payload) => {
        if (!shouldDisplayStream(payload.obligation_id)) {
          return;
        }
        setActive(false);
      }),
    );

    promises.push(
      listenKnownEvent('loop:step_complete', (payload) => {
        if (!shouldDisplayStream(payload.obligation_id)) {
          return;
        }

        const next = { step_number: payload.step_number, model: payload.model };
        setStepInfo(next);
        stepInfoRef.current = next;
      }),
    );

    promises.push(
      listenKnownEvent('loop:obligation_opened', (payload) => {
        setText((current) => `${current}\n--- Obligation opened: ${payload.description} ---\n`);
      }),
    );

    promises.push(
      listenKnownEvent('loop:critic_check', (payload) => {
        const warn = payload.likely_wrong ? ' [LIKELY WRONG]' : '';
        setText((current) => `${current}\n--- Critic check: ${payload.check_description}${warn} ---\n`);
      }),
    );

    promises.push(
      listenKnownEvent('agent:scout_result', (payload) => {
        const label = payload.trigger === 'mid_solve'
          ? `SCOUT (mid-solve: ${payload.obligation_desc || 'stuck obligation'})`
          : 'SCOUT (pre-solve)';
        const srcList = payload.sources.join(', ');
        setText((current) => (
          current
          + `\n--- ${label} ---\n`
          + `${payload.results_count} results from: ${srcList}\n`
          + `${payload.briefing || '(no briefing text)'}\n`
          + '--- end scout ---\n'
        ));
      }),
    );

    return () => {
      promises.forEach((promise) => promise.then((unlisten) => unlisten()));
    };
  }, []);

  useEffect(() => {
    setText('');
  }, [focusObligationId]);

  useEffect(() => {
    const el = bodyRef.current;
    if (el && !collapsed) {
      el.scrollTop = el.scrollHeight;
    }
  }, [text, collapsed]);

  if (!text && !active) {
    return null;
  }

  const roleClass = agentRole ? `agent-${agentRole}` : '';
  const roleLabel = agentRole ? ROLE_LABELS[agentRole] || agentRole.toUpperCase() : null;

  return (
    <div className={`thinking-panel ${active ? 'active' : 'done'}`}>
      <div className="thinking-header" onClick={() => setCollapsed(!collapsed)}>
        <span className="thinking-indicator">
          {active ? <span className="thinking-dot" /> : null}
          {active ? 'Thinking' : 'Last Response'}
        </span>
        {stepInfo && (
          <span className="thinking-meta">
            {roleLabel && <span className={`agent-badge ${roleClass}`}>{roleLabel}</span>}
            {stepInfo.step_number != null
              ? `Step ${stepInfo.step_number} - ${stepInfo.model}`
              : stepInfo.model}
          </span>
        )}
        {focusObligationId && (
          <span className="thinking-focus-badge">FOCUSED</span>
        )}
        <span className="thinking-toggle">{collapsed ? '+' : '-'}</span>
      </div>
      {!collapsed && (
        <pre ref={bodyRef} className="thinking-body">
          {text}
          {active && <span className="thinking-cursor">|</span>}
        </pre>
      )}
    </div>
  );
}
