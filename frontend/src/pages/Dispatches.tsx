import { useEffect, useState } from 'react';
import { listDispatches, getDispatchSummary, updateDispatchStatus } from '../api/dispatches';
import { getOrg } from '../api/orgs';
import { listOrgInvoices, createDispatchInvoice, payInvoice } from '../api/billing';
import { getOrgId } from '../api/auth';
import { IconDispatch, IconClock, IconCheck, IconX } from '../components/Icons';
import { STATUS_TAG_CLASS, NEXT_ACTIONS, formatStatus, type NextAction } from '../lib/dispatchLifecycle';
import type { DispatchOrder, Invoice, PaymentStatus } from '../types';
import './page.css';

const PAYMENT_TAG_CLASS: Record<PaymentStatus, string> = {
  PENDING: 'tag-amber',
  PAID: 'tag-green',
  OVERDUE: 'tag-red',
};

export default function Dispatches() {
  const [orders, setOrders] = useState<DispatchOrder[]>([]);
  const [loading, setLoading] = useState(true);
  const [summaries, setSummaries] = useState<Record<string, string>>({});
  const [loadingId, setLoadingId] = useState<string | null>(null);
  const [openId, setOpenId] = useState<string | null>(null);

  const [actionLoadingId, setActionLoadingId] = useState<string | null>(null);
  const [actionError, setActionError] = useState<Record<string, string>>({});
  const [podDraft, setPodDraft] = useState<{ order: DispatchOrder; action: NextAction } | null>(null);
  const [podReceiver, setPodReceiver] = useState('');
  const [podUrl, setPodUrl] = useState('');

  const [godowns, setGodowns] = useState<{ id: string; name: string }[]>([]);
  const [returnDraft, setReturnDraft] = useState<{ order: DispatchOrder; action: NextAction } | null>(null);
  const [returnGodownId, setReturnGodownId] = useState('');

  const [invoices, setInvoices] = useState<Record<string, Invoice>>({});
  const [invoiceDraftId, setInvoiceDraftId] = useState<string | null>(null);
  const [invAmount, setInvAmount] = useState('');
  const [invDue, setInvDue] = useState('');
  const [invBusyId, setInvBusyId] = useState<string | null>(null);
  const [invError, setInvError] = useState<Record<string, string>>({});

  useEffect(() => {
    listDispatches()
      .then(r => {
        const sorted = [...(r.data ?? [])].sort((a, b) => b.dispatched_at - a.dispatched_at);
        setOrders(sorted);
      })
      .finally(() => setLoading(false));

    const orgId = getOrgId();
    if (orgId) {
      getOrg(orgId)
        .then(r => setGodowns((r.data?.godowns ?? []).map(g => ({ id: g.id, name: g.name }))))
        .catch(() => { /* the godown picker just falls back to the server default */ });
      listOrgInvoices(orgId)
        .then(r => {
          const byDispatch: Record<string, Invoice> = {};
          for (const inv of r.data ?? []) byDispatch[inv.dispatch_id] = inv;
          setInvoices(byDispatch);
        })
        .catch(() => { /* billing is best-effort on this page */ });
    }
  }, []);

  async function handleCreateInvoice(order: DispatchOrder) {
    setInvBusyId(order.id);
    setInvError(prev => ({ ...prev, [order.id]: '' }));
    try {
      const res = await createDispatchInvoice(order.id, { amount: Number(invAmount), dueOn: invDue });
      if (res.data) {
        setInvoices(prev => ({ ...prev, [order.id]: res.data! }));
        setInvoiceDraftId(null);
        setInvAmount('');
        setInvDue('');
      } else {
        setInvError(prev => ({ ...prev, [order.id]: res.message || 'Could not raise the invoice.' }));
      }
    } catch (err) {
      const msg = (err as { response?: { data?: { message?: string } } })?.response?.data?.message;
      setInvError(prev => ({ ...prev, [order.id]: msg || 'Could not raise the invoice.' }));
    } finally {
      setInvBusyId(null);
    }
  }

  async function handlePayInvoice(order: DispatchOrder, invoice: Invoice) {
    setInvBusyId(order.id);
    try {
      const res = await payInvoice(invoice.id);
      if (res.data) setInvoices(prev => ({ ...prev, [order.id]: res.data! }));
    } finally {
      setInvBusyId(null);
    }
  }

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

  async function applyStatus(
    order: DispatchOrder,
    action: NextAction,
    proof?: { receiver_name: string; signature_or_photo_url: string },
    returnToGodownId?: string,
  ) {
    setActionLoadingId(order.id);
    setActionError(prev => ({ ...prev, [order.id]: '' }));
    try {
      const res = await updateDispatchStatus(order.id, action.status, proof, returnToGodownId);
      if (res.data) {
        const updated = res.data;
        setOrders(prev => prev.map(o => (o.id === order.id ? updated : o)));
        setPodDraft(null);
        setPodReceiver('');
        setPodUrl('');
        setReturnDraft(null);
        setReturnGodownId('');
      } else {
        setActionError(prev => ({ ...prev, [order.id]: res.message || 'Status update failed.' }));
      }
    } catch {
      setActionError(prev => ({
        ...prev,
        [order.id]: 'Status update failed. That move may not be allowed from the current status.',
      }));
    } finally {
      setActionLoadingId(null);
    }
  }

  function handleActionClick(order: DispatchOrder, action: NextAction) {
    if (action.requiresProof) {
      setPodDraft({ order, action });
      setPodReceiver('');
      setPodUrl('');
      return;
    }
    if (action.isReturn) {
      setReturnDraft({ order, action });
      setReturnGodownId('');
      return;
    }
    applyStatus(order, action);
  }

  function handleConfirmDelivery() {
    if (!podDraft || !podReceiver.trim() || !podUrl.trim()) return;
    applyStatus(podDraft.order, podDraft.action, {
      receiver_name: podReceiver.trim(),
      signature_or_photo_url: podUrl.trim(),
    });
  }

  function handleConfirmReturn() {
    if (!returnDraft) return;
    applyStatus(returnDraft.order, returnDraft.action, undefined, returnGodownId || undefined);
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
                  <th>Billing</th>
                  <th>Actions</th>
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
                      <td>
                        {o.line_items.map((li, i) => (
                          <div key={i}>
                            {li.stock_description}
                            <span className="muted" style={{ marginLeft: 4 }}>×{li.quantity}</span>
                          </div>
                        ))}
                      </td>
                      <td>
                        <span style={{ fontWeight: 700, fontSize: 14, color: 'var(--text-1)' }}>
                          {o.line_items.reduce((sum, li) => sum + li.quantity, 0)}
                        </span>
                        <span className="muted" style={{ marginLeft: 4 }}>units</span>
                      </td>
                      <td><span className={`status-tag ${STATUS_TAG_CLASS[o.status]}`}>{formatStatus(o.status)}</span></td>
                      <td data-testid="billing-cell">
                        {invoices[o.id] ? (
                          <div style={{ display: 'flex', flexDirection: 'column', gap: 4, alignItems: 'flex-start' }}>
                            <span>
                              <span style={{ fontWeight: 700 }}>{invoices[o.id].amount.toLocaleString()}</span>{' '}
                              <span className={`status-tag ${PAYMENT_TAG_CLASS[invoices[o.id].status]}`}>
                                {invoices[o.id].status}
                              </span>
                            </span>
                            {invoices[o.id].status !== 'PAID' && (
                              <button
                                className="btn btn-sm btn-primary"
                                onClick={() => handlePayInvoice(o, invoices[o.id])}
                                disabled={invBusyId === o.id}
                              >
                                Mark paid
                              </button>
                            )}
                          </div>
                        ) : invoiceDraftId === o.id ? (
                          <span className="muted">see form below</span>
                        ) : (
                          <button
                            className="btn btn-sm"
                            onClick={() => { setInvoiceDraftId(o.id); setInvAmount(''); setInvDue(''); }}
                          >
                            Invoice
                          </button>
                        )}
                        {invError[o.id] && <div className="errortxt" style={{ marginTop: 4 }}>{invError[o.id]}</div>}
                      </td>
                      <td>
                        {NEXT_ACTIONS[o.status] ? (
                          <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                            {NEXT_ACTIONS[o.status]!.map(action => (
                              <button
                                key={action.status}
                                className={`btn btn-sm ${action.variant === 'danger' ? 'btn-danger' : 'btn-primary'}`}
                                onClick={() => handleActionClick(o, action)}
                                disabled={actionLoadingId === o.id}
                              >
                                {action.label}
                              </button>
                            ))}
                          </div>
                        ) : (
                          <span className="muted">—</span>
                        )}
                        {actionError[o.id] && (
                          <div className="errortxt" style={{ marginTop: 4 }}>{actionError[o.id]}</div>
                        )}
                      </td>
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
                    {invoiceDraftId === o.id && (
                      <tr key={`${o.id}-invoice`}>
                        <td colSpan={9} style={{ padding: '0 16px 14px' }}>
                          <div style={{ display: 'flex', gap: 12, alignItems: 'flex-end', flexWrap: 'wrap', padding: 12, background: 'var(--surface)', borderRadius: 8 }}>
                            <div className="field" style={{ marginBottom: 0 }}>
                              <label htmlFor={`inv-amount-${o.id}`}>Freight Amount</label>
                              <input
                                id={`inv-amount-${o.id}`}
                                type="number"
                                value={invAmount}
                                onChange={e => setInvAmount(e.target.value)}
                                placeholder="e.g. 4500"
                              />
                            </div>
                            <div className="field" style={{ marginBottom: 0 }}>
                              <label htmlFor={`inv-due-${o.id}`}>Due Date</label>
                              <input
                                id={`inv-due-${o.id}`}
                                type="date"
                                value={invDue}
                                onChange={e => setInvDue(e.target.value)}
                              />
                            </div>
                            <button
                              className="btn btn-primary btn-sm"
                              onClick={() => handleCreateInvoice(o)}
                              disabled={!invAmount || !invDue || invBusyId === o.id}
                            >
                              <IconCheck size={13} /> Raise Invoice
                            </button>
                            <button className="btn btn-ghost btn-sm" onClick={() => setInvoiceDraftId(null)}>
                              <IconX size={13} /> Cancel
                            </button>
                          </div>
                        </td>
                      </tr>
                    )}
                    {podDraft?.order.id === o.id && (
                      <tr key={`${o.id}-pod`}>
                        <td colSpan={9} style={{ padding: '0 16px 14px' }}>
                          <div style={{ display: 'flex', gap: 12, alignItems: 'flex-end', flexWrap: 'wrap', padding: 12, background: 'var(--surface)', borderRadius: 8 }}>
                            <div className="field" style={{ marginBottom: 0 }}>
                              <label htmlFor={`pod-receiver-${o.id}`}>Receiver Name</label>
                              <input
                                id={`pod-receiver-${o.id}`}
                                value={podReceiver}
                                onChange={e => setPodReceiver(e.target.value)}
                                placeholder="Who signed for it?"
                              />
                            </div>
                            <div className="field" style={{ marginBottom: 0 }}>
                              <label htmlFor={`pod-url-${o.id}`}>Signature / Photo URL</label>
                              <input
                                id={`pod-url-${o.id}`}
                                value={podUrl}
                                onChange={e => setPodUrl(e.target.value)}
                                placeholder="https://…"
                              />
                            </div>
                            <button
                              className="btn btn-primary btn-sm"
                              onClick={handleConfirmDelivery}
                              disabled={!podReceiver.trim() || !podUrl.trim() || actionLoadingId === o.id}
                            >
                              <IconCheck size={13} /> Confirm Delivery
                            </button>
                            <button className="btn btn-ghost btn-sm" onClick={() => setPodDraft(null)}>
                              <IconX size={13} /> Cancel
                            </button>
                          </div>
                        </td>
                      </tr>
                    )}
                    {returnDraft?.order.id === o.id && (
                      <tr key={`${o.id}-return`}>
                        <td colSpan={9} style={{ padding: '0 16px 14px' }}>
                          <div style={{ display: 'flex', gap: 12, alignItems: 'flex-end', flexWrap: 'wrap', padding: 12, background: 'var(--surface)', borderRadius: 8 }}>
                            <div className="field" style={{ marginBottom: 0 }}>
                              <label htmlFor={`return-godown-${o.id}`}>Return stock to</label>
                              <select
                                id={`return-godown-${o.id}`}
                                value={returnGodownId}
                                onChange={e => setReturnGodownId(e.target.value)}
                              >
                                <option value="">Auto — a godown that stocks these items</option>
                                {godowns.map(g => (
                                  <option key={g.id} value={g.id}>{g.name}</option>
                                ))}
                              </select>
                            </div>
                            <span className="muted" style={{ fontSize: 12, paddingBottom: 8 }}>
                              The shipment's stock is credited back into the godown.
                            </span>
                            <button
                              className="btn btn-danger btn-sm"
                              onClick={handleConfirmReturn}
                              disabled={actionLoadingId === o.id}
                            >
                              <IconCheck size={13} /> Confirm Return
                            </button>
                            <button className="btn btn-ghost btn-sm" onClick={() => setReturnDraft(null)}>
                              <IconX size={13} /> Cancel
                            </button>
                          </div>
                        </td>
                      </tr>
                    )}
                    {openId === o.id && (
                      <tr key={`${o.id}-summary`}>
                        <td colSpan={9} style={{ padding: '0 16px 14px', background: 'var(--ai-summary-bg, var(--surface))' }}>
                          <div className="ai-summary-card">
                            {o.status_history.length > 0 && (
                              <div style={{ marginBottom: 12 }}>
                                <h4 style={{ fontSize: 12, textTransform: 'uppercase', letterSpacing: '.05em', color: 'var(--text-3)', marginBottom: 6 }}>
                                  Status History
                                </h4>
                                <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'flex', flexDirection: 'column', gap: 4 }}>
                                  {o.status_history.map((ev, i) => (
                                    <li key={i} style={{ fontSize: 13, display: 'flex', gap: 8, alignItems: 'center' }}>
                                      <span className={`status-tag ${STATUS_TAG_CLASS[ev.status]}`}>{formatStatus(ev.status)}</span>
                                      <span className="muted">{new Date(ev.changed_at * 1000).toLocaleString()}</span>
                                    </li>
                                  ))}
                                </ul>
                              </div>
                            )}
                            {o.proof_of_delivery && (
                              <div style={{ marginBottom: 12, fontSize: 13 }}>
                                <h4 style={{ fontSize: 12, textTransform: 'uppercase', letterSpacing: '.05em', color: 'var(--text-3)', marginBottom: 6 }}>
                                  Proof of Delivery
                                </h4>
                                <p style={{ margin: 0 }}>
                                  Received by <strong>{o.proof_of_delivery.receiver_name}</strong> on{' '}
                                  {new Date(o.proof_of_delivery.delivered_at * 1000).toLocaleString()}
                                </p>
                                <a href={o.proof_of_delivery.signature_or_photo_url} target="_blank" rel="noreferrer">
                                  View signature/photo
                                </a>
                              </div>
                            )}
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
