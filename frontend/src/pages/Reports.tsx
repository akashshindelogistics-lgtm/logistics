import { useEffect, useState } from 'react';
import { getOpsReport, getOpsReportSummary } from '../api/reports';
import { getOrgId } from '../api/auth';
import { IconChart, IconTruck, IconDispatch, IconClock, IconCheck, IconPackage } from '../components/Icons';
import type { HiredTransport, OpsReport } from '../types';
import './page.css';
import './Dashboard.css';

const fmtHours = (h: number | null) =>
  h == null ? '—' : h < 48 ? `${h.toFixed(1)} h` : `${(h / 24).toFixed(1)} d`;
const fmtPct = (p: number | null) => (p == null ? '—' : `${p.toFixed(1)}%`);

export default function Reports() {
  const [report, setReport] = useState<OpsReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [summary, setSummary] = useState<string | null>(null);
  const [summaryLoading, setSummaryLoading] = useState(false);

  useEffect(() => {
    const orgId = getOrgId();
    if (!orgId) {
      setLoading(false);
      setError(true);
      return;
    }
    getOpsReport(orgId)
      .then(r => setReport(r.data ?? null))
      .catch(() => setError(true))
      .finally(() => setLoading(false));
  }, []);

  async function handleExplainReport() {
    const orgId = getOrgId();
    if (!orgId) return;

    setSummaryLoading(true);
    try {
      const r = await getOpsReportSummary(orgId);
      setSummary(r.data || r.message || 'No summary available.');
    } catch {
      setSummary('Could not generate summary. Ensure ANTHROPIC_API_KEY is set on the server.');
    } finally {
      setSummaryLoading(false);
    }
  }

  const tiles = report && [
    { label: 'Fleet utilization', value: fmtPct(report.vehicle_utilization.utilization_percent),
      sub: `${report.vehicle_utilization.vehicles_on_active_trip} of ${report.vehicle_utilization.total_vehicles} on a trip`,
      Icon: IconTruck, cls: 'card-green' },
    { label: 'Delivered', value: report.delivery_performance.delivered_count.toLocaleString(),
      sub: `${report.delivery_performance.returned_count} returned`,
      Icon: IconCheck, cls: 'card-blue' },
    { label: 'Avg. time to deliver', value: fmtHours(report.delivery_performance.avg_hours_to_deliver),
      sub: 'creation → delivered', Icon: IconClock, cls: 'card-purple' },
    { label: 'On-time rate', value: fmtPct(report.delivery_performance.on_time_rate_percent),
      sub: 'delivered within 72 h', Icon: IconDispatch, cls: 'card-amber' },
  ];

  const maxVolume = Math.max(1, ...(report?.dispatch_volume.map(p => p.count) ?? [1]));

  return (
    <div className="page">
      <div className="page-header">
        <div className="page-title-group">
          <h1>Reports</h1>
          <p>Operational snapshot for your organization</p>
        </div>
      </div>

      {loading ? (
        <div className="stat-grid">
          {[1, 2, 3, 4].map(i => <div key={i} className="skeleton" style={{ height: 120, borderRadius: 14 }} />)}
        </div>
      ) : error || !report ? (
        <div className="empty-state">
          <div className="empty-state-icon"><IconChart size={26} /></div>
          <h3>No report available</h3>
          <p>Reports need an organization. Once you have dispatches and vehicles, the numbers show up here.</p>
        </div>
      ) : (
        <>
          <div className="section-card" style={{ marginBottom: 20 }}>
            <div className="section-card-header">
              <span className="section-card-title">✦ Explain this report</span>
              <button
                type="button"
                className="btn btn-sm btn-ai"
                onClick={handleExplainReport}
                disabled={summaryLoading}
              >
                {summaryLoading ? 'Generating…' : summary ? 'Regenerate' : 'Explain this report'}
              </button>
            </div>
            {(summaryLoading || summary) && (
              <div style={{ padding: '0 16px 16px' }}>
                <div className="ai-summary-card">
                  {summaryLoading ? (
                    <div className="ai-summary-loading">
                      <span className="ai-pulse" />
                      Generating briefing…
                    </div>
                  ) : (
                    <p className="ai-summary-text">{summary}</p>
                  )}
                </div>
              </div>
            )}
          </div>

          <div className="stat-grid">
            {tiles!.map(({ label, value, sub, Icon, cls }) => (
              <div key={label} className={`stat-card ${cls}`}>
                <div className="stat-card-top">
                  <div className="stat-icon"><Icon size={20} /></div>
                </div>
                <div>
                  <div className="stat-value">{value}</div>
                  <div className="stat-label">{label}</div>
                  <div className="muted" style={{ marginTop: 4 }}>{sub}</div>
                </div>
              </div>
            ))}
          </div>

          <div className="section-card" style={{ marginTop: 20 }}>
            <div className="section-card-header">
              <span className="section-card-title"><IconChart size={15} />Dispatch volume &middot; last 14 days</span>
              <span className="muted">{report.units_dispatched_recently.toLocaleString()} units dispatched in 30 days</span>
            </div>
            <div style={{ padding: '20px', display: 'flex', alignItems: 'flex-end', gap: 6, height: 160 }}>
              {report.dispatch_volume.map(p => (
                <div key={p.date} style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 6, height: '100%' }}
                     title={`${p.date}: ${p.count}`}>
                  <div style={{ flex: 1, width: '100%', display: 'flex', alignItems: 'flex-end' }}>
                    <div style={{
                      width: '100%',
                      height: `${(p.count / maxVolume) * 100}%`,
                      minHeight: p.count > 0 ? 4 : 0,
                      background: 'var(--blue)',
                      borderRadius: '4px 4px 0 0',
                    }} />
                  </div>
                  <span className="muted" style={{ fontSize: 10 }}>{p.date.slice(5)}</span>
                </div>
              ))}
            </div>
          </div>

          <div className="section-card" style={{ marginTop: 20 }}>
            <div className="section-card-header">
              <span className="section-card-title"><IconPackage size={15} />Godown inventory</span>
              <span className="badge">{report.godown_inventory.length}</span>
            </div>
            {report.godown_inventory.length === 0 ? (
              <div style={{ padding: '20px' }} className="muted">No godowns yet.</div>
            ) : (
              <div className="table-wrap">
                <table>
                  <thead>
                    <tr><th>Godown</th><th>Units on hand</th><th>Distinct items</th><th>Capacity used</th></tr>
                  </thead>
                  <tbody>
                    {report.godown_inventory.map(g => (
                      <tr key={g.godown_id}>
                        <td className="entity-name">{g.godown_name}</td>
                        <td>{g.units_on_hand.toLocaleString()}</td>
                        <td className="muted">{g.distinct_items}</td>
                        <td>{g.capacity_used_percent == null
                          ? <span className="muted">no cap</span>
                          : `${g.capacity_used_percent.toFixed(1)}%`}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          {report.hired_transport && <HiredTransportSection hired={report.hired_transport} />}
        </>
      )}
    </div>
  );
}

function HiredTransportSection({ hired }: { hired: HiredTransport }) {
  if (hired.hired_dispatches === 0 && hired.vendors.length === 0) {
    return (
      <div className="section-card" style={{ marginTop: 20 }}>
        <div className="section-card-header">
          <span className="section-card-title"><IconTruck size={15} />Hired transport</span>
        </div>
        <div style={{ padding: 20 }} className="muted">No dispatches have gone out on hired trucks yet.</div>
      </div>
    );
  }
  const total = hired.own_dispatches + hired.hired_dispatches;
  return (
    <div className="section-card" style={{ marginTop: 20 }} data-testid="hired-transport">
      <div className="section-card-header">
        <span className="section-card-title"><IconTruck size={15} />Hired transport</span>
        <span className="muted">
          {hired.hired_dispatches} of {total} dispatches on hired trucks ({fmtPct(hired.hired_share_percent)})
          {hired.awaiting_truck > 0 && ` · ${hired.awaiting_truck} awaiting a truck`}
        </span>
      </div>
      <div style={{ padding: '4px 20px 12px', display: 'flex', gap: 28, flexWrap: 'wrap' }}>
        <div><div className="muted">Hire cost</div><div style={{ fontWeight: 700, fontSize: 18 }}>{hired.hire_cost_total.toLocaleString()}</div></div>
        <div><div className="muted">Paid to vendors</div><div style={{ fontWeight: 700, fontSize: 18 }}>{hired.paid_to_vendors.toLocaleString()}</div></div>
        <div><div className="muted">Still owed</div><div style={{ fontWeight: 700, fontSize: 18 }} data-testid="owed-to-vendors">{hired.outstanding_to_vendors.toLocaleString()}</div></div>
      </div>
      {hired.vendors.length > 0 && (
        <div className="table-wrap">
          <table>
            <thead>
              <tr><th>Vendor</th><th>Hires</th><th>Hire cost</th><th>Paid</th><th>Outstanding</th></tr>
            </thead>
            <tbody>
              {hired.vendors.map(v => (
                <tr key={v.vendor_id}>
                  <td className="entity-name">{v.vendor_name}</td>
                  <td className="muted">{v.hires}</td>
                  <td>{v.hire_cost.toLocaleString()}</td>
                  <td>{v.paid.toLocaleString()}</td>
                  <td>{v.outstanding > 0 ? <strong>{v.outstanding.toLocaleString()}</strong> : <span className="muted">—</span>}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {hired.hire_margins.length > 0 && (
        <div className="table-wrap" style={{ marginTop: 12 }}>
          <table>
            <thead>
              <tr><th>Hired truck</th><th>Vendor</th><th>Invoiced</th><th>Hire cost</th><th>Margin</th></tr>
            </thead>
            <tbody>
              {hired.hire_margins.map(m => (
                <tr key={m.hire_id} data-testid="hire-margin-row">
                  <td className="entity-name">
                    {m.registration_number ?? '—'}
                    {m.trip_id && <span className="badge tag-blue" style={{ marginLeft: 6 }}>Trip</span>}
                  </td>
                  <td className="muted">{m.vendor_name}</td>
                  <td>
                    {m.invoiced.toLocaleString()}
                    {m.invoiced_dispatches < m.dispatches && (
                      <span className="muted" style={{ marginLeft: 6 }} title="Some dispatches on this hire haven't been invoiced yet">
                        ({m.invoiced_dispatches}/{m.dispatches} invoiced)
                      </span>
                    )}
                  </td>
                  <td>{m.hire_cost.toLocaleString()}</td>
                  <td style={{ fontWeight: 700, color: m.margin < 0 ? 'var(--red)' : 'var(--green)' }}>
                    {m.margin.toLocaleString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
