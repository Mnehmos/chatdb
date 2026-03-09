import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../services/tauri', () => ({
  exportTrainingData: vi.fn(),
  getExportDirectory: vi.fn(),
}));

describe('ExportPanel', () => {
  beforeEach(async () => {
    const api = await import('../../services/tauri');
    vi.mocked(api.getExportDirectory).mockResolvedValue('exports-dir');
    vi.mocked(api.exportTrainingData).mockReset();
  });

  it('loads the export directory and exports all selected types for a problem scope', async () => {
    const api = await import('../../services/tauri');
    vi.mocked(api.exportTrainingData)
      .mockResolvedValueOnce(3)
      .mockResolvedValueOnce(2)
      .mockResolvedValueOnce(1);

    const { ExportPanel } = await import('./ExportPanel');

    render(<ExportPanel problemId="123456789abc" />);

    fireEvent.click(screen.getByText('Export Training Data'));

    await waitFor(() => {
      expect(screen.getByDisplayValue('exports-dir')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('checkbox', { name: /Planning Decisions/i }));
    fireEvent.click(screen.getByRole('checkbox', { name: /Contrastive Pairs/i }));
    fireEvent.click(screen.getByRole('button', { name: 'Export 3 type(s)' }));

    await waitFor(() => {
      expect(api.exportTrainingData).toHaveBeenCalledTimes(3);
    });

    expect(api.exportTrainingData).toHaveBeenNthCalledWith(
      1,
      'step_verification',
      'exports-dir/step_verification_12345678.jsonl',
      '123456789abc',
    );
    expect(api.exportTrainingData).toHaveBeenNthCalledWith(
      2,
      'planning',
      'exports-dir/planning_12345678.jsonl',
      '123456789abc',
    );
    expect(api.exportTrainingData).toHaveBeenNthCalledWith(
      3,
      'contrastive',
      'exports-dir/contrastive_12345678.jsonl',
      '123456789abc',
    );
    expect(screen.getByText('3 records exported')).toBeInTheDocument();
    expect(screen.getByText('2 records exported')).toBeInTheDocument();
    expect(screen.getByText('1 records exported')).toBeInTheDocument();
  });

  it('disables export when no types are selected', async () => {
    const { ExportPanel } = await import('./ExportPanel');

    render(<ExportPanel />);

    fireEvent.click(screen.getByText('Export Training Data'));
    fireEvent.click(screen.getByRole('checkbox', { name: /Step Verification Pairs/i }));

    expect(screen.getByRole('button', { name: 'Export 0 type(s)' })).toBeDisabled();
  });
});
