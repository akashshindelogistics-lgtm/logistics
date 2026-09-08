import { useEffect, useState } from 'react';
import { getOpsReport } from '../api/reports';
import { getOrgId } from '../api/auth';
import { IconChart, IconTruck, IconDispatch, IconClock, IconCheck, IconPackage } from '../components/Icons';
import type { OpsReport } from '../types';
import './page.css';
import './Dashboard.css';

const fmtHours = (h: number | null) =>
  h == null ? '—' : h < 48 ? `${h.toFixed(1)} h` : `${(h / 24).toFixed(1)} d`;
const fmtPct = (p: number | null) => (p == null ? '—' : `${p.toFixed(1)}%`);

export default function Reports() {
  const [report, setReport] = useState<OpsReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);

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
        </>
      )}
    </div>
  );
}
