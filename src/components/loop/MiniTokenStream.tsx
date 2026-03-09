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
  obligationId: string;
  focused?: boolean;
  onFocus?: () => void;
}

export function MiniTokenStream({ obligationId, focused, onFocus }: Props) {
  const [text, setText] = useState('');
  const [active, setActive] = useState(false);
  const [role, setRole] = useState<string | null>(null);
  const bodyRef = useRef<HTMLPreElement>(null);
  const MAX_CHARS = 2000;

  useEffect(() => {
    const promises: Promise<() => void>[] = [];

    promises.push(
      listenKnownEvent('loop:thinking_start', (payload) => {
        if (payload.obligation_id !== obligationId) {
          return;
        }

        const newRole = payload.agent_role || 'solver';
        const label = ROLE_LABELS[newRole] || newRole.toUpperCase();
        setText((current) => {
          const divider = current ? `\n-- ${label} --\n` : `-- ${label} --\n`;
          return current + divider;
        });
        setActive(true);
        setRole(newRole);
      }),
    );

    promises.push(
      listenKnownEvent('loop:token', (payload) => {
        if (payload.obligation_id !== obligationId) {
          return;
        }

        setText((current) => {
          const next = current + payload.text;
          return next.length > MAX_CHARS ? next.slice(-MAX_CHARS) : next;
        });
        if (payload.agent_role) {
          setRole(payload.agent_role);
        }
      }),
    );

    promises.push(
      listenKnownEvent('loop:thinking_end', (payload) => {
        if (payload.obligation_id === obligationId) {
          setActive(false);
        }
      }),
    );

    return () => {
      promises.forEach((promise) => promise.then((unlisten) => unlisten()));
    };
  }, [obligationId]);

  useEffect(() => {
    const el = bodyRef.current;
    if (el) {
      el.scrollTop = el.scrollHeight;
    }
  }, [text]);

  const roleLabel = role ? (ROLE_LABELS[role] || role.toUpperCase()) : null;

  return (
    <div
      className={`mini-token-stream${active ? ' active' : ''}${focused ? ' focused' : ''}`}
      onClick={onFocus}
      title="Click to focus main view"
    >
      <div className="mini-stream-header">
        {active && <span className="mini-dot" />}
        {roleLabel && <span className={`mini-role agent-${role}`}>{roleLabel}</span>}
        {!active && !text && <span className="mini-idle">waiting...</span>}
      </div>
      {text && (
        <pre ref={bodyRef} className="mini-stream-body">
          {text}
          {active && <span className="thinking-cursor">|</span>}
        </pre>
      )}
    </div>
  );
}
