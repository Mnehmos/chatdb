import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

let problemStoreState: { createProblem: ReturnType<typeof vi.fn> };

const useProblemStoreMock = vi.fn(() => problemStoreState);

vi.mock('../../stores/problemStore', () => ({
  useProblemStore: useProblemStoreMock,
}));

function deferredPromise<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

describe('ProblemInput', () => {
  beforeEach(() => {
    problemStoreState = {
      createProblem: vi.fn().mockResolvedValue(undefined),
    };
    useProblemStoreMock.mockImplementation(() => problemStoreState);
  });

  it('disables Start Solving until the statement contains non-whitespace text', async () => {
    const { ProblemInput } = await import('./ProblemInput');

    render(<ProblemInput />);

    const button = screen.getByRole('button', { name: 'Start Solving' });
    const textarea = screen.getByPlaceholderText(
      'Prove that the sum of the first n natural numbers is n(n+1)/2',
    );

    expect(button).toBeDisabled();

    fireEvent.change(textarea, { target: { value: '   ' } });
    expect(button).toBeDisabled();

    fireEvent.change(textarea, { target: { value: 'Prove x^2 >= 0' } });
    expect(button).toBeEnabled();
  });

  it('trims the statement and prevents duplicate submits while creation is in flight', async () => {
    const pending = deferredPromise<void>();
    problemStoreState = {
      createProblem: vi.fn().mockReturnValue(pending.promise),
    };
    useProblemStoreMock.mockImplementation(() => problemStoreState);

    const { ProblemInput } = await import('./ProblemInput');

    render(<ProblemInput />);

    const textarea = screen.getByPlaceholderText(
      'Prove that the sum of the first n natural numbers is n(n+1)/2',
    );
    const domainSelect = screen.getByRole('combobox');
    const button = screen.getByRole('button', { name: 'Start Solving' });

    fireEvent.change(textarea, { target: { value: '  Prove x^2 >= 0  ' } });
    fireEvent.change(domainSelect, { target: { value: 'number_theory' } });
    fireEvent.click(button);
    fireEvent.click(button);

    expect(problemStoreState.createProblem).toHaveBeenCalledTimes(1);
    expect(problemStoreState.createProblem).toHaveBeenCalledWith('Prove x^2 >= 0', 'number_theory');
    expect(button).toBeDisabled();

    pending.resolve();

    await waitFor(() => {
      expect(textarea).toHaveValue('');
      expect(button).toBeDisabled();
    });
  });
});
