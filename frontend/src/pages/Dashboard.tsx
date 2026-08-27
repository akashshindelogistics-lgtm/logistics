import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { listOrgs } from '../api/orgs';
import { listVehicles } from '../api/vehicles';
import { listCustomers } from '../api/customers';
import { listDispatches } from '../api/dispatches';
import './Dashboard.css';

export default function Dashboard() {
  const [counts, setCounts] = useState({ orgs: 0, vehicles: 0, customers: 0, dispatches: 0 });
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([listOrgs(), listVehicles(), listCustomers(), listDispatches()])
      .then(([orgs, vehicles, customers, dispatches]) => {
        setCounts({
          orgs: orgs.data?.length ?? 0,
          vehicles: vehicles.data?.length ?? 0,
          customers: customers.data?.length ?? 0,
          dispatches: dispatches.data?.length ?? 0,
        });
      })
      .finally(() => setLoading(false));
  }, []);

  const cards = [
    { label: 'Organizations', value: counts.orgs, to: '/orgs', color: '#0ea5e9' },
    { label: 'Vehicles', value: counts.vehicles, to: '/vehicles', color: '#10b981' },
    { label: 'Customers', value: counts.customers, to: '/customers', color: '#f59e0b' },
    { label: 'Dispatches', value: counts.dispatches, to: '/dispatches', color: '#8b5cf6' },
  ];

  return (
    <div className="page">
      <h1>Dashboard</h1>
      {loading ? (
        <p className="muted">Loading...</p>
      ) : (
        <div className="stat-grid">
          {cards.map(c => (
            <Link key={c.to} to={c.to} className="stat-card" style={{ borderTop: `4px solid ${c.color}` }}>
              <div className="stat-value" style={{ color: c.color }}>{c.value}</div>
              <div className="stat-label">{c.label}</div>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
