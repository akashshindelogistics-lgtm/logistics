import { useState } from 'react';
import { askAssistant, reindexAssistant } from '../api/assistant';
import { isLoggedIn, getOrgId } from '../api/auth';
import type { AssistantSource } from '../types';
import '../pages/page.css';
import './AssistantWidget.css';

interface Turn {
  question: string;
  answer: string;
  sources: AssistantSource[];
}

/** Summarize a turn's sources as "Based on N facts: 2 notifications, 1 vehicle document". */
function sourceSummary(sources: AssistantSource[]): string | null {
  if (sources.length === 0) return null;
  const counts = new Map<string, number>();
  for (const s of sources) counts.set(s.kind, (counts.get(s.kind) ?? 0) + 1);
  const parts = [...counts.entries()].map(([kind, n]) => `${n} ${kind.replace('_', ' ')}${n > 1 ? 's' : ''}`);
  return `Based on ${sources.length} fact${sources.length > 1 ? 's' : ''}: ${parts.join(', ')}`;
}

export default function AssistantWidget() {
  const [open, setOpen] = useState(false);
  const [question, setQuestion] = useState('');
  const [turns, setTurns] = useState<Turn[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reindexing, setReindexing] = useState(false);
  const [reindexStatus, setReindexStatus] = useState<string | null>(null);

  if (!isLoggedIn()) return null;

  const handleReindex = async () => {
    const orgId = getOrgId();
    if (!orgId) return;

    setReindexing(true);
    setReindexStatus(null);
    try {
      const res = await reindexAssistant(orgId);
      setReindexStatus(res.data ? `Reindexed ${res.data.chunks_indexed} fact(s).` : res.message);
    } catch {
      setReindexStatus('Could not reindex right now.');
    } finally {
      setReindexing(false);
    }
  };

  const handleAsk = async (e: React.FormEvent) => {
    e.preventDefault();
    const orgId = getOrgId();
    const q = question.trim();
    if (!q || !orgId) return;

    setLoading(true);
    setError(null);
    try {
      const res = await askAssistant(orgId, q);
      if (res.data) {
        setTurns(prev => [{ question: q, answer: res.data!.answer, sources: res.data!.sources }, ...prev]);
        setQuestion('');
      } else {
        setError(res.message || 'No answer available.');
      }
    } catch {
      setError('Could not reach the assistant. Ensure ANTHROPIC_API_KEY is set on the server.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="assistant-widget">
      {open && (
        <div className="assistant-panel" role="dialog" aria-label="Ask your data">
          <div className="assistant-panel-header">
            <span>✦ Ask your data</span>
            <button
              type="button"
              className="assistant-close"
              aria-label="Close assistant"
              onClick={() => setOpen(false)}
            >
              ✕
            </button>
          </div>

          <div className="assistant-history">
            {turns.length === 0 && !loading && (
              <p className="muted assistant-empty">
                Ask a question about your organization's own data — dispatches, notifications, vehicle
                compliance documents.
              </p>
            )}
            {loading && (
              <div className="ai-summary-loading">
                <span className="ai-pulse" />
                Thinking…
              </div>
            )}
            {error && <p className="errortxt">{error}</p>}
            {turns.map((t, i) => (
              <div className="assistant-turn" key={i}>
                <p className="assistant-question">{t.question}</p>
                <div className="ai-summary-card">
                  <p className="ai-summary-text">{t.answer}</p>
                  {sourceSummary(t.sources) && (
                    <details className="assistant-sources">
                      <summary>{sourceSummary(t.sources)}</summary>
                      <ul>
                        {t.sources.map((s, j) => (
                          <li key={j}>
                            <span className="badge">{s.kind}</span> {s.excerpt}
                          </li>
                        ))}
                      </ul>
                    </details>
                  )}
                </div>
              </div>
            ))}
          </div>

          <form className="assistant-input-row" onSubmit={handleAsk}>
            <input
              type="text"
              placeholder="Ask a question…"
              value={question}
              onChange={e => setQuestion(e.target.value)}
              aria-label="Question for the assistant"
            />
            <button className="btn btn-ai" type="submit" disabled={loading || !question.trim()}>
              Ask
            </button>
          </form>

          <div className="assistant-panel-footer">
            <button
              type="button"
              className="assistant-reindex-link"
              onClick={handleReindex}
              disabled={reindexing}
            >
              {reindexing ? 'Reindexing…' : 'Reindex my data'}
            </button>
            {reindexStatus && <span className="assistant-reindex-status">{reindexStatus}</span>}
          </div>
        </div>
      )}

      <button
        type="button"
        className="assistant-launcher"
        aria-label={open ? 'Collapse assistant' : 'Open assistant'}
        onClick={() => setOpen(o => !o)}
      >
        ✦
      </button>
    </div>
  );
}
