import { useEffect, useState } from 'react';
import { listDispatches } from '../api/dispatches';
import type { DispatchOrder } from '../types';
import './page.css';

export default function Dispatches() {
  const [orders, setOrders] = useState<DispatchOrder[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    listDispatches()
      .then(r => setOrders(r.data ?? []))
      .finally(() => setLoading(false));
  }, []);

  return (
    <div className="page">
      <div className="page-header">
        <h1>Dispatch Orders</h1>
        <span className="badge">{orders.length} total</span>
      </div>

      {loading ? (
        <p className="muted">Loading...</p>
      ) : orders.length === 0 ? (
        <p className="muted">No dispatch orders yet. Create one from an organization's detail page.</p>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>ID</th>
                <th>Vehicle</th>
                <th>Stock</th>
                <th>Qty</th>
                <th>Status</th>
                <th>Dispatched At</th>
              </tr>
            </thead>
            <tbody>
              {orders.map(o => (
                <tr key={o.id}>
                  <td className="mono">{o.id.slice(0, 8)}…</td>
                  <td>{o.vehicle_registration_number}</td>
                  <td>{o.stock_description}</td>
                  <td>{o.quantity}</td>
                  <td><span className={`status-badge status-${o.status.toLowerCase()}`}>{o.status}</span></td>
                  <td>{new Date(o.dispatched_at * 1000).toLocaleString()}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
