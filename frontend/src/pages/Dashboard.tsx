import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { listOrgs } from '../api/orgs';
import { listVehicles } from '../api/vehicles';
import { listCustomers } from '../api/customers';
import { listDispatches } from '../api/dispatches';
import { IconBuilding, IconTruck, IconUsers, IconDispatch } from '../components/Icons';
import type { DispatchOrder } from '../types';
import './page.css';
import './Dashboard.css';

export default function Dashboard() {
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

  const cards = [
    { label: 'Organizations', value: counts.orgs, to: '/orgs', Icon: IconBuilding, cls: 'card-blue' },
    { label: 'Fleet Vehicles', value: counts.vehicles, to: '/vehicles', Icon: IconTruck, cls: 'card-green' },
    { label: 'Customers', value: counts.customers, to: '/customers', Icon: IconUsers, cls: 'card-amber' },
    { label: 'Dispatches', value: counts.dispatches, to: '/dispatches', Icon: IconDispatch, cls: 'card-purple' },
  ];

  return (
    <div className="page">
      <div className="dash-hero">
        <div>
          <h1 className="dash-hero-title">Good morning 👋</h1>
          <p className="dash-hero-sub">Here's what's happening across your logistics network today.</p>
        </div>
      </div>

      <div className="stat-grid">
        {cards.map(({ label, value, to, Icon, cls }) => (
          <Link key={to} to={to} className={`stat-card ${cls}`}>
            <div className="stat-card-top">
              <div className="stat-icon">
                <Icon size={20} />
              </div>
            </div>
            <div>
              {loading
                ? <div className="skeleton" style={{ width: 60, height: 36, marginBottom: 6 }} />
                : <div className="stat-value">{value.toLocaleString()}</div>
              }
              <div className="stat-label">{label}</div>
            </div>
          </Link>
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
            <div className="empty-state-icon"><IconDispatch size={26} /></div>
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
              <tbody>
                {recent.map(o => (
                  <tr key={o.id}>
                    <td><span className="mono">{o.id.slice(0, 8)}…</span></td>
                    <td className="entity-name">{o.vehicle_registration_number}</td>
                    <td>{o.stock_description}</td>
                    <td><strong>{o.quantity}</strong></td>
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
