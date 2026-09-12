import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import AssistantWidget from './AssistantWidget';
import * as assistantApi from '../api/assistant';

vi.mock('../api/assistant');
vi.mock('../api/auth', () => ({
  isLoggedIn: vi.fn(() => true),
  getOrgId: () => 'org1',
}));

import { isLoggedIn } from '../api/auth';

const ok = <T,>(data: T) => ({ success: true, message: '', data });

describe('AssistantWidget', () => {
  beforeEach(() => vi.resetAllMocks());

  it('renders nothing when the user is not logged in', () => {
    vi.mocked(isLoggedIn).mockReturnValue(false);
    const { container } = render(<AssistantWidget />);
    expect(container).toBeEmptyDOMElement();
  });

  it('is collapsed by default, showing only the launcher button', () => {
    vi.mocked(isLoggedIn).mockReturnValue(true);
    render(<AssistantWidget />);
    expect(screen.getByRole('button', { name: /open assistant/i })).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('opens the panel, asks a question, and shows the answer with its source summary', async () => {
    vi.mocked(isLoggedIn).mockReturnValue(true);
    vi.mocked(assistantApi.askAssistant).mockResolvedValue(
      ok({
        answer: 'Order abcd1234 was delivered yesterday.',
        sources: [
          { kind: 'notification', source_id: 'n1', excerpt: 'delivered' },
          { kind: 'notification', source_id: 'n2', excerpt: 'dispatched' },
        ],
      }),
    );

    const user = userEvent.setup();
    render(<AssistantWidget />);

    await user.click(screen.getByRole('button', { name: /open assistant/i }));
    expect(screen.getByRole('dialog', { name: /ask your data/i })).toBeInTheDocument();

    await user.type(screen.getByLabelText(/question for the assistant/i), 'what happened to order abcd1234?');
    await user.click(screen.getByRole('button', { name: /^ask$/i }));

    expect(assistantApi.askAssistant).toHaveBeenCalledWith('org1', 'what happened to order abcd1234?');
    expect(await screen.findByText('Order abcd1234 was delivered yesterday.')).toBeInTheDocument();
    expect(screen.getByText(/based on 2 facts: 2 notifications/i)).toBeInTheDocument();

    // The question box clears after a successful ask.
    expect(screen.getByLabelText(/question for the assistant/i)).toHaveValue('');
  });

  it('shows an error message when the assistant call fails', async () => {
    vi.mocked(isLoggedIn).mockReturnValue(true);
    vi.mocked(assistantApi.askAssistant).mockRejectedValue(new Error('network error'));

    const user = userEvent.setup();
    render(<AssistantWidget />);
    await user.click(screen.getByRole('button', { name: /open assistant/i }));
    await user.type(screen.getByLabelText(/question for the assistant/i), 'anything?');
    await user.click(screen.getByRole('button', { name: /^ask$/i }));

    expect(await screen.findByText(/could not reach the assistant/i)).toBeInTheDocument();
  });

  it('reindexes on demand and shows how many chunks were rebuilt', async () => {
    vi.mocked(isLoggedIn).mockReturnValue(true);
    vi.mocked(assistantApi.reindexAssistant).mockResolvedValue(ok({ chunks_indexed: 7 }));

    const user = userEvent.setup();
    render(<AssistantWidget />);
    await user.click(screen.getByRole('button', { name: /open assistant/i }));
    await user.click(screen.getByRole('button', { name: /reindex my data/i }));

    expect(assistantApi.reindexAssistant).toHaveBeenCalledWith('org1');
    expect(await screen.findByText(/reindexed 7 fact\(s\)/i)).toBeInTheDocument();
  });

  it('shows a status message when reindexing fails', async () => {
    vi.mocked(isLoggedIn).mockReturnValue(true);
    vi.mocked(assistantApi.reindexAssistant).mockRejectedValue(new Error('network error'));

    const user = userEvent.setup();
    render(<AssistantWidget />);
    await user.click(screen.getByRole('button', { name: /open assistant/i }));
    await user.click(screen.getByRole('button', { name: /reindex my data/i }));

    expect(await screen.findByText(/could not reindex right now/i)).toBeInTheDocument();
  });
});
