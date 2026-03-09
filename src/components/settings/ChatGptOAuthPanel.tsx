import { useState, useEffect, useRef } from 'react';
import { chatgptOAuthStart, chatgptOAuthPoll, chatgptOAuthStatus, chatgptOAuthLogout } from '../../services/tauri';

interface Props {
  open: boolean;
  onClose: () => void;
}

export function ChatGptOAuthPanel({ open, onClose }: Props) {
  const [authenticated, setAuthenticated] = useState(false);
  const [expiresAt, setExpiresAt] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Device flow state
  const [userCode, setUserCode] = useState<string | null>(null);
  const [verificationUrl, setVerificationUrl] = useState<string | null>(null);
  const [, setDeviceAuthId] = useState<string | null>(null);
  const [pollInterval, setPollInterval] = useState(5);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const checkStatus = async () => {
    try {
      const s = await chatgptOAuthStatus();
      setAuthenticated(s.authenticated);
      setExpiresAt(s.expires_at);
    } catch {
      // command might not exist yet
    }
  };

  useEffect(() => {
    if (open) checkStatus();
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, [open]);

  if (!open) return null;

  const startFlow = async () => {
    setLoading(true);
    setError(null);
    try {
      const info = await chatgptOAuthStart();
      setUserCode(info.user_code);
      setVerificationUrl(info.verification_url);
      setDeviceAuthId(info.device_auth_id);
      setPollInterval(info.interval || 5);

      // Start polling
      if (pollRef.current) clearInterval(pollRef.current);
      pollRef.current = setInterval(async () => {
        try {
          const result = await chatgptOAuthPoll(info.device_auth_id, info.user_code);
          if (result.complete) {
            if (pollRef.current) clearInterval(pollRef.current);
            pollRef.current = null;
            setUserCode(null);
            setDeviceAuthId(null);
            await checkStatus();
          } else if (result.error) {
            if (pollRef.current) clearInterval(pollRef.current);
            pollRef.current = null;
            setError(result.error);
            setUserCode(null);
            setDeviceAuthId(null);
          }
        } catch (e) {
          setError(String(e));
        }
      }, (info.interval || 5) * 1000);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleLogout = async () => {
    await chatgptOAuthLogout();
    setAuthenticated(false);
    setExpiresAt(null);
  };

  const expiryLabel = expiresAt
    ? new Date(expiresAt * 1000).toLocaleTimeString()
    : null;

  return (
    <div className="profile-panel-overlay" onClick={onClose}>
      <div className="profile-panel" onClick={(e) => e.stopPropagation()} style={{ maxWidth: '440px' }}>
        <div className="profile-panel-header">
          <h3>ChatGPT OAuth</h3>
          <button className="btn btn-sm" onClick={onClose}>Close</button>
        </div>

        <section className="agent-config-section">
          <p style={{ fontSize: '11px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)', lineHeight: '1.5' }}>
            Sign in with your ChatGPT Plus/Pro subscription to use GPT-4o, o3, and GPT-5 without separate API credits.
          </p>

          {authenticated ? (
            <div style={{ marginTop: '12px' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
                <span style={{ color: 'var(--green)', fontFamily: 'var(--font-mono)', fontSize: '12px', fontWeight: 600 }}>
                  Authenticated
                </span>
                {expiryLabel && (
                  <span style={{ color: 'var(--text-muted)', fontFamily: 'var(--font-mono)', fontSize: '10px' }}>
                    expires {expiryLabel}
                  </span>
                )}
              </div>
              <p style={{ fontSize: '10px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)', marginBottom: '8px' }}>
                Select any "ChatGPT Sub" model in the agent profile to use your subscription.
              </p>
              <button className="btn btn-sm btn-danger" onClick={handleLogout}>Logout</button>
            </div>
          ) : userCode ? (
            <div style={{ marginTop: '12px', textAlign: 'center' }}>
              <p style={{ fontSize: '12px', color: 'var(--text-primary)', fontFamily: 'var(--font-mono)', marginBottom: '12px' }}>
                Open this URL and enter the code:
              </p>
              <div style={{
                background: 'var(--bg-tertiary)', border: '1px solid var(--border)',
                borderRadius: 'var(--radius)', padding: '16px', marginBottom: '12px',
              }}>
                <div style={{
                  fontSize: '24px', fontWeight: 700, fontFamily: 'var(--font-mono)',
                  color: 'var(--accent)', letterSpacing: '4px', marginBottom: '8px',
                }}>
                  {userCode}
                </div>
                <a
                  href={verificationUrl || '#'}
                  target="_blank"
                  rel="noopener noreferrer"
                  style={{
                    fontSize: '11px', fontFamily: 'var(--font-mono)',
                    color: 'var(--accent)', textDecoration: 'underline',
                  }}
                >
                  {verificationUrl}
                </a>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', justifyContent: 'center' }}>
                <span className="waiting-ellipsis" style={{ fontSize: '14px' }}>...</span>
                <span style={{ fontSize: '10px', color: 'var(--text-muted)', fontFamily: 'var(--font-mono)' }}>
                  Waiting for authentication (polling every {pollInterval}s)
                </span>
              </div>
            </div>
          ) : (
            <div style={{ marginTop: '12px' }}>
              <button
                className="btn btn-primary"
                onClick={startFlow}
                disabled={loading}
                style={{ width: '100%', padding: '10px' }}
              >
                {loading ? 'Starting...' : 'Sign in with ChatGPT'}
              </button>
            </div>
          )}

          {error && (
            <div style={{
              marginTop: '8px', padding: '8px 12px', background: '#3a1111',
              border: '1px solid #ff4444', borderRadius: '4px', fontSize: '11px',
              fontFamily: 'var(--font-mono)', color: '#ffaaaa',
            }}>
              {error}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
