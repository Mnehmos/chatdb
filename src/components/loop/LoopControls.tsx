import { useState } from 'react';
import { useLoopStore, MODEL_PRESETS } from '../../stores/loopStore';
import { useProblemStore } from '../../stores/problemStore';
import { AfterActionReport } from '../analytics/AfterActionReport';
import { AgentProfilePanel } from '../settings/AgentProfilePanel';
import { ResearchApiPanel } from '../settings/ResearchApiPanel';
import { ChatGptOAuthPanel } from '../settings/ChatGptOAuthPanel';
import { runManualReview } from '../../services/tauri';

function isProofComplete(steps: { proposal_type?: string; verified: boolean }[]): boolean {
  return steps.some(s => s.verified && s.proposal_type === 'conclusion');
}

export function LoopControls() {
  const {
    status, startSolve, continueSolve, pause, stop, currentStep, steps,
    selectedModel, activeProfile,
    attemptNumber, maxAttempts, warmupStatus,
    adversaryModel, reviewerModel, discernerModel,
    lastError,
  } = useLoopStore();
  const { activeProblem } = useProblemStore();
  const [showAAR, setShowAAR] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showResearch, setShowResearch] = useState(false);
  const [showChatGptAuth, setShowChatGptAuth] = useState(false);
  const [reviewing, setReviewing] = useState(false);
  if (!activeProblem) return null;

  const hasSteps = steps.length > 0;
  const proofDone = isProofComplete(steps);

  // Build a compact summary of current config for display
  const solverLabel = MODEL_PRESETS.find(p => p.config.provider === selectedModel.provider && p.config.model === selectedModel.model)?.label || selectedModel.model;
  const reviewerLabel = reviewerModel
    ? MODEL_PRESETS.find(p => p.config.provider === reviewerModel.provider && p.config.model === reviewerModel.model)?.label || 'custom'
    : null;
  const adversaryLabel = adversaryModel
    ? MODEL_PRESETS.find(p => p.config.provider === adversaryModel.provider && p.config.model === adversaryModel.model)?.label || 'custom'
    : null;
  const discernerLabel = discernerModel
    ? MODEL_PRESETS.find(p => p.config.provider === discernerModel.provider && p.config.model === discernerModel.model)?.label || 'custom'
    : null;

  return (
    <>
      <div className="loop-controls">
        <div className="loop-status">
          <span className={`status-dot ${status}`} />
          <span className="status-text">{status}</span>
          {(currentStep > 0 || steps.length > 0) && <span className="step-counter">Step {currentStep || steps.length} ({steps.length}V+R)</span>}
          {attemptNumber > 1 && <span className="attempt-counter">Attempt {attemptNumber}/{maxAttempts}</span>}
          {warmupStatus === 'waiting' && (
            <span className="warmup-indicator waiting">
              <span className="warmup-spinner" /> Warming up Lean...
            </span>
          )}
          {warmupStatus === 'timeout' && (
            <span className="warmup-indicator timeout">Lean warmup timed out</span>
          )}
          {activeProfile && <span className="active-profile-badge">{activeProfile.name}</span>}
          {lastError && <span role="alert" className="loop-error">{lastError}</span>}
        </div>
        <div className="loop-actions">
          {status !== 'running' && (
            <>
              <button className="btn btn-sm settings-gear" onClick={() => setShowSettings(true)}
                title="Agent configuration">
                Settings
              </button>
              <button className="btn btn-sm" onClick={() => setShowResearch(true)}
                title="Research API configuration">
                Research
              </button>
              <button className="btn btn-sm" onClick={() => setShowChatGptAuth(true)}
                title="ChatGPT subscription OAuth">
                ChatGPT
              </button>
              <span className="config-summary">
                {solverLabel}
                {reviewerLabel && <> / R: {reviewerLabel}</>}
                {adversaryLabel && <> / A: {adversaryLabel}</>}
                {discernerLabel && <> / D: {discernerLabel}</>}
              </span>
            </>
          )}
          {status === 'idle' && !hasSteps && <button className="btn btn-primary" onClick={() => startSolve(activeProblem.id)}>Solve</button>}
          {status === 'idle' && hasSteps && (
            <>
              {!proofDone && <button className="btn btn-primary" onClick={() => continueSolve(activeProblem.id)}>Continue</button>}
              <button className="btn btn-secondary" onClick={() => startSolve(activeProblem.id)}>Retry</button>
            </>
          )}
          {status === 'running' && <><button className="btn btn-secondary" onClick={pause}>Pause</button><button className="btn btn-danger" onClick={stop}>Stop</button></>}
          {status === 'paused' && <><button className="btn btn-primary" onClick={() => continueSolve(activeProblem.id)}>Resume</button><button className="btn btn-danger" onClick={stop}>Stop</button></>}
          {status === 'finished' && (
            <>
              {!proofDone && <button className="btn btn-primary" onClick={() => continueSolve(activeProblem.id)}>Continue</button>}
              <button className="btn btn-secondary" onClick={() => startSolve(activeProblem.id)}>Retry</button>
            </>
          )}
          {hasSteps && status !== 'running' && (
            <>
              <button
                className="btn btn-warning"
                disabled={reviewing}
                onClick={async () => {
                  setReviewing(true);
                  try {
                    const config = useLoopStore.getState().fullConfig;
                    await runManualReview(activeProblem.id, config);
                  } catch (e) {
                    console.error('Manual review failed:', e);
                  } finally {
                    setReviewing(false);
                  }
                }}
              >
                {reviewing ? 'Reviewing...' : 'Review'}
              </button>
              <button className="btn btn-accent" onClick={() => setShowAAR(true)}>After Action</button>
            </>
          )}
        </div>
      </div>
      {showAAR && <AfterActionReport problemId={activeProblem.id} onClose={() => setShowAAR(false)} />}
      <AgentProfilePanel open={showSettings} onClose={() => setShowSettings(false)} />
      <ResearchApiPanel open={showResearch} onClose={() => setShowResearch(false)} />
      <ChatGptOAuthPanel open={showChatGptAuth} onClose={() => setShowChatGptAuth(false)} />
    </>
  );
}
