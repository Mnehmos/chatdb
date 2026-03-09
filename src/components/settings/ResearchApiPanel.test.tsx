import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../services/tauri', () => ({
  listResearchApiKeys: vi.fn(),
  setResearchApiKey: vi.fn(),
  deleteResearchApiKey: vi.fn(),
  toggleResearchApiKey: vi.fn(),
  researchSearch: vi.fn(),
  researchSources: vi.fn(),
}));

describe('ResearchApiPanel', () => {
  beforeEach(async () => {
    const api = await import('../../services/tauri');
    vi.mocked(api.listResearchApiKeys).mockResolvedValue([
      { service: 'semantic_scholar', key_masked: 'sk-***', active: true } as any,
    ]);
    vi.mocked(api.researchSources).mockResolvedValue({
      sources: [
        { id: 'arxiv', name: 'arXiv', capabilities: ['search'], requires_key: false },
        { id: 'semantic_scholar', name: 'Semantic Scholar', capabilities: ['search'], requires_key: true },
      ],
    } as any);
    vi.mocked(api.setResearchApiKey).mockResolvedValue({} as any);
    vi.mocked(api.deleteResearchApiKey).mockResolvedValue(undefined);
    vi.mocked(api.toggleResearchApiKey).mockResolvedValue(undefined);
    vi.mocked(api.researchSearch).mockResolvedValue({ query: 'parity', results: [{ title: 'Parity paper' }] });
  });

  it('does not render when closed', async () => {
    const { ResearchApiPanel } = await import('./ResearchApiPanel');

    const { container } = render(<ResearchApiPanel open={false} onClose={vi.fn()} />);

    expect(container).toBeEmptyDOMElement();
  });

  it('loads sources and keys, supports adding a key, and runs a test search', async () => {
    const api = await import('../../services/tauri');
    const { ResearchApiPanel } = await import('./ResearchApiPanel');

    render(<ResearchApiPanel open={true} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('arXiv')).toBeInTheDocument();
    });

    expect(screen.getByText('sk-***')).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole('button', { name: '+ Add' })[0]!);
    fireEvent.change(screen.getByPlaceholderText('Paste API key...'), {
      target: { value: 'secret-key' },
    });
    fireEvent.change(screen.getByPlaceholderText('Label (optional)'), {
      target: { value: 'Primary' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(api.setResearchApiKey).toHaveBeenCalled();
    });

    expect(api.setResearchApiKey).toHaveBeenCalledWith('wolfram_alpha', 'secret-key', 'Primary');

    fireEvent.change(screen.getByPlaceholderText('Search query...'), {
      target: { value: 'parity' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Test' }));

    await waitFor(() => {
      expect(api.researchSearch).toHaveBeenCalledWith('arxiv', 'parity', 3);
    });

    expect(screen.getByText(/Parity paper/)).toBeInTheDocument();
  });
});
