import { useEffect, useState } from 'react';
import { listDispatches, getDispatchSummary } from '../api/dispatches';
import { IconDispatch, IconClock } from '../components/Icons';
import type { DispatchOrder } from '../types';
import './page.css';

export default function Dispatches() {
  const [orders, setOrders] = useState<DispatchOrder[]>([]);
  const [loading, setLoading] = useState(true);
  const [summaries, setSummaries] = useState<Record<string, string>>({});
  const [loadingId, setLoadingId] = useState<string | null>(null);
  const [openId, setOpenId] = useState<string | null>(null);

  useEffect(() => {
    listDispatches()
      .then(r => {
        const sorted = [...(r.data ?? [])].sort((a, b) => b.dispatched_at - a.dispatched_at);
        setOrders(sorted);
      })
      .finally(() => setLoading(false));
  }, []);

  async function handleAiStatus(orderId: string) {
    if (openId === orderId) {
      setOpenId(null);
      return;
    }
    setOpenId(orderId);
    if (summaries[orderId]) return;

    setLoadingId(orderId);
    try {
      const r = await getDispatchSummary(orderId);
      if (r.data) setSummaries(prev => ({ ...prev, [orderId]: r.data! }));
      else setSummaries(prev => ({ ...prev, [orderId]: r.message || 'No summary available.' }));
    } catch {
      setSummaries(prev => ({ ...prev, [orderId]: 'Could not generate summary. Ensure ANTHROPIC_API_KEY is set on the server.' }));
    } finally {
      setLoadingId(null);
    }
  }

  return (
    <div className="page">
      <div className="page-header">
        <div className="page-title-group">
          <h1>Dispatch Orders</h1>
          <p>Full history of all stock dispatches</p>
        </div>
        {!loading && <span className="badge">{orders.length} orders</span>}
      </div>

      <div className="table-card">
        <div className="table-toolbar">
          <span className="table-toolbar-title">
            All Orders <span style={{ color: 'var(--text-3)', fontWeight: 400 }}>· sorted by latest</span>
          </span>
        </div>

        {loading ? (
          <div style={{ padding: '20px', display: 'flex', flexDirection: 'column', gap: 14 }}>
            {[1,2,3,4,5].map(i => <div key={i} className="skeleton" style={{ height: 20 }} />)}
          </div>
        ) : orders.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon"><IconDispatch size={26} /></div>
            <h3>No dispatch orders yet</h3>
            <p>Dispatch orders are created from an organization's detail page when stock is sent to a customer.</p>
          </div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Order ID</th>
                  <th>Vehicle</th>
                  <th>Stock Item</th>
                  <th>Qty</th>
                  <th>Status</th>
                  <th>Dispatched At</th>
                  <th>AI Status</th>
                </tr>
              </thead>
              <tbody>
                {orders.map(o => (
                  <>
                    <tr key={o.id}>
                      <td><span className="mono">{o.id.slice(0, 8)}…</span></td>
                      <td>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                          <div style={{ width: 28, height: 28, borderRadius: 6, background: 'var(--green-bg)', color: 'var(--green)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
                            <IconDispatch size={13} />
                          </div>
                          <span className="entity-name">{o.vehicle_registration_number}</span>
                        </div>
                      </td>
                      <td>{o.stock_description}</td>
                      <td>
                        <span style={{ fontWeight: 700, fontSize: 14, color: 'var(--text-1)' }}>{o.quantity}</span>
                        <span className="muted" style={{ marginLeft: 4 }}>units</span>
                      </td>
                      <td><span className="status-tag tag-green">{o.status}</span></td>
                      <td>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
                          <IconClock size={12} className="muted" />
                          <span className="muted">{new Date(o.dispatched_at * 1000).toLocaleString()}</span>
                        </div>
                      </td>
                      <td>
                        <button
                          className={`btn btn-sm ${openId === o.id ? 'btn-ghost' : 'btn-ai'}`}
                          onClick={() => handleAiStatus(o.id)}
                          disabled={loadingId === o.id}
                          style={{ whiteSpace: 'nowrap' }}
                        >
                          {loadingId === o.id
                            ? 'Thinking…'
                            : openId === o.id
                              ? 'Hide'
                              : '✦ AI Status'}
                        </button>
                      </td>
                    </tr>
                    {openId === o.id && (
                      <tr key={`${o.id}-summary`}>
                        <td colSpan={7} style={{ padding: '0 16px 14px', background: 'var(--ai-summary-bg, var(--surface))' }}>
                          <div className="ai-summary-card">
                            {loadingId === o.id || !summaries[o.id] ? (
                              <div className="ai-summary-loading">
                                <span className="ai-pulse" />
                                Generating status summary…
                              </div>
                            ) : (
                              <p className="ai-summary-text">{summaries[o.id]}</p>
                            )}
                          </div>
                        </td>
                      </tr>
                    )}
                  </>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
