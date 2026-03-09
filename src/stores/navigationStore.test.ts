import { beforeEach, describe, expect, it } from 'vitest';

import { useNavigationStore } from './navigationStore';

describe('navigationStore', () => {
  beforeEach(() => {
    useNavigationStore.setState({
      level: 'library',
      problemId: null,
      attemptId: null,
      stepId: null,
      obligationId: null,
    });
  });

  it('walks back up the hierarchy from step to attempt to workspace to library', () => {
    const store = useNavigationStore.getState();

    store.goToStep('problem-1', 'attempt-1', 'step-1');
    expect(useNavigationStore.getState()).toMatchObject({
      level: 'step',
      problemId: 'problem-1',
      attemptId: 'attempt-1',
      stepId: 'step-1',
      obligationId: null,
    });

    useNavigationStore.getState().goBack();
    expect(useNavigationStore.getState()).toMatchObject({
      level: 'attempt',
      problemId: 'problem-1',
      attemptId: 'attempt-1',
      stepId: null,
    });

    useNavigationStore.getState().goBack();
    expect(useNavigationStore.getState()).toMatchObject({
      level: 'workspace',
      problemId: 'problem-1',
      attemptId: null,
      stepId: null,
    });

    useNavigationStore.getState().goBack();
    expect(useNavigationStore.getState()).toMatchObject({
      level: 'library',
      problemId: null,
      attemptId: null,
      stepId: null,
      obligationId: null,
    });
  });

  it('returns from an obligation view to its attempt without losing the current problem', () => {
    useNavigationStore.getState().goToObligation('problem-9', 'attempt-4', 'obl-3');

    useNavigationStore.getState().goBack();

    expect(useNavigationStore.getState()).toMatchObject({
      level: 'attempt',
      problemId: 'problem-9',
      attemptId: 'attempt-4',
      obligationId: null,
      stepId: null,
    });
  });
});
