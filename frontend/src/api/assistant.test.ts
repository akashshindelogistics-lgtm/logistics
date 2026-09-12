import { describe, it, expect, vi, beforeEach } from 'vitest';
import api from './client';
import { askAssistant } from './assistant';

vi.mock('./client', () => ({ default: { post: vi.fn() } }));

const envelope = <T,>(data: T) => ({ data: { success: true, message: '', data } });

describe('assistant api client', () => {
  beforeEach(() => vi.resetAllMocks());

  it('askAssistant POSTs the question to /orgs/{id}/assistant/ask and unwraps the envelope', async () => {
    vi.mocked(api.post).mockResolvedValue(
      envelope({ answer: 'Your order was delivered.', sources: [{ kind: 'notification', source_id: 'n1', excerpt: 'delivered' }] }),
    );
    const res = await askAssistant('org1', 'what happened to my order?');
    expect(api.post).toHaveBeenCalledWith('/orgs/org1/assistant/ask', { question: 'what happened to my order?' });
    expect(res.data?.answer).toBe('Your order was delivered.');
    expect(res.data?.sources).toHaveLength(1);
  });
});
