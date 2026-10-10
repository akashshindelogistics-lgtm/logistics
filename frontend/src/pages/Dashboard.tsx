import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { listOrgs } from '../api/orgs';
import { listVehicles } from '../api/vehicles';
import { listCustomers } from '../api/customers';
import { listDispatches } from '../api/dispatches';
import Icon3D, { type Icon3DName } from '../components/Icon3D';
import type { DispatchOrder } from '../types';
import './page.css';
import './Dashboard.css';
import { m } from 'motion/react';
import { DURATION, EASE_OUT, useCountUp, useListAnimation } from '../lib/motion';

// Stat cards are links that fade up one after another on first paint.
const MotionLink = m.create(Link);

function StatValue({ value }: { value: number }) {
  return <div className="stat-value">{useCountUp(value).toLocaleString()}</div>;
}

export default function Dashboard() {
  const rowsRef = useListAnimation();
  const [counts, setCounts] = useState({ orgs: 0, vehicles: 0, customers: 0, dispatches: 0 });
  const [recent, setRecent] = useState<DispatchOrder[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([listOrgs(), listVehicles(), listCustomers(), listDispatches()])
      .then(([orgs, vehicles, customers, dispatches]) => {
        const orders = dispatches.data ?? [];
        setCounts({
          orgs: orgs.data?.length ?? 0,
          vehicles: vehicles.data?.length ?? 0,
          customers: customers.data?.length ?? 0,
          dispatches: orders.length,
        });
        setRecent([...orders].sort((a, b) => b.dispatched_at - a.dispatched_at).slice(0, 5));
      })
      .finally(() => setLoading(false));
  }, []);

  const cards: { label: string; value: number; to: string; icon: Icon3DName; cls: string }[] = [
    { label: 'Organizations', value: counts.orgs, to: '/orgs', icon: 'building', cls: 'card-blue' },
    { label: 'Fleet Vehicles', value: counts.vehicles, to: '/vehicles', icon: 'truck', cls: 'card-green' },
    { label: 'Customers', value: counts.customers, to: '/customers', icon: 'store', cls: 'card-amber' },
    { label: 'Dispatches', value: counts.dispatches, to: '/dispatches', icon: 'package', cls: 'card-purple' },
  ];

  return (
    <div className="page">
      <div className="dash-hero">
        <div>
          <h1 className="dash-hero-title">Good morning 👋</h1>
          <p className="dash-hero-sub">Here's what's happening across your logistics network today.</p>
        </div>
        <div className="dash-hero-art" aria-hidden="true">
          <Icon3D name="map" size={64} className="dash-hero-art-back" />
          <Icon3D name="lorry" size={88} className="dash-hero-art-main" float />
          <Icon3D name="package" size={52} className="dash-hero-art-front" />
        </div>
      </div>

      <div className="stat-grid">
        {cards.map(({ label, value, to, icon, cls }, i) => (
          <MotionLink
            key={to}
            to={to}
            className={`stat-card ${cls}`}
            initial={{ opacity: 0, y: 14 }}
            animate={{ opacity: 1, y: 0, transition: { delay: i * 0.07, duration: DURATION.slow, ease: EASE_OUT } }}
            whileHover={{ y: -2, transition: { duration: DURATION.fast } }}
          >
            <div className="stat-card-top">
              <div className="stat-icon stat-icon-3d">
                <Icon3D name={icon} size={36} float />
              </div>
            </div>
            <div>
              {loading
                ? <div className="skeleton" style={{ width: 60, height: 36, marginBottom: 6 }} />
                : <StatValue value={value} />
              }
              <div className="stat-label">{label}</div>
            </div>
          </MotionLink>
        ))}
      </div>

      <div className="table-card">
        <div className="table-toolbar">
          <span className="table-toolbar-title">Recent Dispatches</span>
          <Link to="/dispatches" className="btn btn-ghost btn-sm">View all →</Link>
        </div>
        {loading ? (
          <div style={{ padding: '24px 20px', display: 'flex', flexDirection: 'column', gap: 12 }}>
            {[1,2,3].map(i => <div key={i} className="skeleton" style={{ height: 20 }} />)}
          </div>
        ) : recent.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon empty-state-icon-3d"><Icon3D name="outbox" size={48} /></div>
            <h3>No dispatches yet</h3>
            <p>Dispatch orders will appear here once you send stock to customers.</p>
          </div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Order ID</th><th>Vehicle</th><th>Stock</th><th>Qty</th><th>Status</th><th>Time</th>
                </tr>
              </thead>
              <tbody ref={rowsRef}>
                {recent.map(o => (
                  <tr key={o.id}>
                    <td><span className="mono">{o.id.slice(0, 8)}…</span></td>
                    <td className="entity-name">{o.vehicle_registration_number ?? <span className="muted">Awaiting vehicle</span>}</td>
                    <td>
                      {o.line_items.length === 1
                        ? o.line_items[0].stock_description
                        : `${o.line_items.length} items`}
                    </td>
                    <td><strong>{o.line_items.reduce((sum, li) => sum + li.quantity, 0)}</strong></td>
                    <td><span className="status-tag tag-green">{o.status}</span></td>
                    <td className="muted">{new Date(o.dispatched_at * 1000).toLocaleString()}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
