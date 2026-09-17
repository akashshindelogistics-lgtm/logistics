import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { listOrgTrips, createTrip, type TripStopInput } from '../api/trips';
import { listCustomers } from '../api/customers';
import { listVehicles } from '../api/vehicles';
import { getOrgId } from '../api/auth';
import { IconDispatch, IconPlus, IconX, IconTruck } from '../components/Icons';
import { STATUS_TAG_CLASS, formatStatus } from '../lib/dispatchLifecycle';
import LocationMap, { type MapPin } from '../components/LocationMap';
import type { Customer, Trip, TripStatus, Vehicle } from '../types';
import './page.css';

const TRIP_TAG: Record<TripStatus, string> = {
  PLANNED: 'tag-amber',
  IN_PROGRESS: 'tag-blue',
  COMPLETED: 'tag-green',
};

interface StopDraft {
  customerId: string;
  description: string;
  quantity: string;
}
const emptyStop = (): StopDraft => ({ customerId: '', description: '', quantity: '' });

export default function Trips() {
  const [trips, setTrips] = useState<Trip[]>([]);
  const [customers, setCustomers] = useState<Customer[]>([]);
  const [vehicles, setVehicles] = useState<Vehicle[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [stops, setStops] = useState<StopDraft[]>([emptyStop(), emptyStop()]);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState('');
  const [mapOpenId, setMapOpenId] = useState<string | null>(null);

  const load = () => {
    const orgId = getOrgId();
    if (!orgId) { setLoading(false); return; }
    Promise.all([listOrgTrips(orgId), listCustomers(), listVehicles()])
      .then(([t, c, v]) => { setTrips(t.data ?? []); setCustomers(c.data ?? []); setVehicles(v.data ?? []); })
      .finally(() => setLoading(false));
  };
  useEffect(load, []);

  const custName = (id: string) => customers.find(c => c.id === id)?.name ?? id.slice(0, 8);
  const setStop = (i: number, patch: Partial<StopDraft>) =>
    setStops(prev => prev.map((s, j) => (j === i ? { ...s, ...patch } : s)));

  // The trip's assigned vehicle (if its GPS tracker has reported a location)
  // plus every stop whose customer has a location on file — so the map shows
  // where the truck is relative to where it's still headed.
  const tripPins = (t: Trip): MapPin[] => {
    const pins: MapPin[] = [];
    const vehicle = vehicles.find(v => v.registration_number === t.vehicle_registration_number);
    if (vehicle?.location) {
      pins.push({
        lat: vehicle.location.latitude,
        lng: vehicle.location.longitude,
        label: `🚚 ${vehicle.registration_number}`,
        detail: `Last reported ${new Date(vehicle.location.timestamp * 1000).toLocaleString()}`,
      });
    }
    for (const s of t.stops) {
      const c = customers.find(cust => cust.id === s.customer_id);
      if (c?.location) {
        pins.push({
          lat: c.location.latitude,
          lng: c.location.longitude,
          label: `Stop ${s.stop_sequence}: ${c.name}`,
          detail: formatStatus(s.status),
        });
      }
    }
    return pins;
  };

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    const orgId = getOrgId();
    if (!orgId) return;
    const filled = stops.filter(s => s.customerId && s.description && Number(s.quantity) > 0);
    if (filled.length < 2) {
      setError('A trip needs at least two complete stops.');
      return;
    }
    if (new Set(filled.map(s => s.customerId)).size !== filled.length) {
      setError('Each customer can only appear once in a trip.');
      return;
    }
    const payload: TripStopInput[] = filled.map(s => ({
      customer_id: s.customerId,
      line_items: [{ stock_description: s.description, requested_quantity: Number(s.quantity) }],
    }));
    setSubmitting(true);
    setError('');
    try {
      await createTrip(orgId, payload);
      setStops([emptyStop(), emptyStop()]);
      setShowForm(false);
      load();
    } catch (err) {
      const msg = (err as { response?: { data?: { message?: string } } }).response?.data?.message;
      setError(msg || 'Could not plan the trip.');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="page">
      <div className="page-header">
        <div className="page-title-group">
          <h1>Multi-stop Trips</h1>
          <p>One vehicle serving several customer orders in a sequence</p>
        </div>
        <button className="btn btn-primary" onClick={() => { setShowForm(!showForm); setError(''); }}>
          {showForm ? <><IconX size={14} />Cancel</> : <><IconPlus size={14} />Plan a Trip</>}
        </button>
      </div>

      {showForm && (
        <div className="form-panel">
          <h2>Plan a Multi-stop Trip</h2>
          <form onSubmit={handleCreate}>
            {stops.map((s, i) => (
              <div key={i} style={{ display: 'flex', gap: 10, alignItems: 'flex-end', marginBottom: 10, flexWrap: 'wrap' }}>
                <div className="field" style={{ marginBottom: 0, minWidth: 180 }}>
                  <label htmlFor={`stop-cust-${i}`}>Stop {i + 1} — customer</label>
                  <select id={`stop-cust-${i}`} value={s.customerId} onChange={e => setStop(i, { customerId: e.target.value })}>
                    <option value="">Select a customer…</option>
                    {customers.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
                  </select>
                </div>
                <div className="field" style={{ marginBottom: 0, minWidth: 160 }}>
                  <label htmlFor={`stop-desc-${i}`}>Stock item</label>
                  <input id={`stop-desc-${i}`} value={s.description} onChange={e => setStop(i, { description: e.target.value })} placeholder="e.g. Cement Bags" />
                </div>
                <div className="field" style={{ marginBottom: 0, width: 110 }}>
                  <label htmlFor={`stop-qty-${i}`}>Quantity</label>
                  <input id={`stop-qty-${i}`} type="number" min="1" value={s.quantity} onChange={e => setStop(i, { quantity: e.target.value })} />
                </div>
                {stops.length > 2 && (
                  <button type="button" className="btn btn-ghost btn-sm" onClick={() => setStops(prev => prev.filter((_, j) => j !== i))} aria-label={`Remove stop ${i + 1}`}>
                    <IconX size={12} />
                  </button>
                )}
              </div>
            ))}
            <button type="button" className="btn btn-ghost btn-sm" onClick={() => setStops(prev => [...prev, emptyStop()])} style={{ marginBottom: 12 }}>
              <IconPlus size={12} />Add another stop
            </button>
            {error && <div className="errortxt" style={{ marginBottom: 12 }}>{error}</div>}
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="btn btn-primary" type="submit" disabled={submitting}>
                {submitting ? 'Planning…' : 'Plan Trip'}
              </button>
            </div>
          </form>
        </div>
      )}

      <div className="table-card">
        <div className="table-toolbar">
          <span className="table-toolbar-title">All Trips</span>
          {!loading && <span className="badge">{trips.length}</span>}
        </div>
        {loading ? (
          <div style={{ padding: 20 }}><div className="skeleton" style={{ height: 20 }} /></div>
        ) : trips.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon"><IconDispatch size={26} /></div>
            <h3>No trips yet</h3>
            <p>Plan a multi-stop trip to send one vehicle to several customers in a row.</p>
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 14, padding: 16 }}>
            {trips.map(t => (
              <div key={t.id} className="section-card">
                <div className="section-card-header">
                  <span className="section-card-title">
                    <IconTruck size={15} />{t.vehicle_registration_number}
                    <span className="mono muted" style={{ marginLeft: 8 }}>{t.id.slice(0, 8)}…</span>
                  </span>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                    <span className={`status-tag ${TRIP_TAG[t.status]}`}>{t.status.replace('_', ' ')}</span>
                    <button
                      className="btn btn-ghost btn-sm"
                      onClick={() => setMapOpenId(mapOpenId === t.id ? null : t.id)}
                      aria-label={mapOpenId === t.id ? `Hide map for ${t.vehicle_registration_number}` : `Route map for ${t.vehicle_registration_number}`}
                    >
                      {mapOpenId === t.id ? 'Hide map' : '📍 Route map'}
                    </button>
                  </div>
                </div>
                {mapOpenId === t.id && (
                  tripPins(t).length > 0 ? (
                    <LocationMap pins={tripPins(t)} height="280px" />
                  ) : (
                    <p className="muted" style={{ padding: '0 16px' }}>
                      No location data yet — the vehicle hasn't reported GPS coordinates and none of this trip's customers have a location on file.
                    </p>
                  )
                )}
                <div className="table-wrap">
                  <table>
                    <thead>
                      <tr><th>Stop</th><th>Customer</th><th>Items</th><th>Status</th><th></th></tr>
                    </thead>
                    <tbody>
                      {t.stops.map(s => (
                        <tr key={s.id}>
                          <td>{s.stop_sequence}</td>
                          <td className="entity-name">{custName(s.customer_id)}</td>
                          <td className="muted">
                            {s.line_items.map(li => `${li.stock_description} ×${li.quantity}`).join(', ')}
                          </td>
                          <td><span className={`status-tag ${STATUS_TAG_CLASS[s.status]}`}>{formatStatus(s.status)}</span></td>
                          <td style={{ textAlign: 'right' }}>
                            <Link to="/dispatches" className="btn btn-ghost btn-sm">Manage →</Link>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
