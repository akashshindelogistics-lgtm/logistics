import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { getOrg, dispatchStock } from '../api/orgs';
import { addVehicle, deleteVehicle } from '../api/vehicles';
import { listCustomers } from '../api/customers';
import type { Organization, Customer } from '../types';
import LocationMap, { type MapPin } from '../components/LocationMap';
import { IconBuilding, IconTruck, IconPackage, IconDispatch, IconPlus, IconTrash, IconPin, IconChevron, IconCheck } from '../components/Icons';
import './page.css';

export default function OrganizationDetail() {
  const { id } = useParams<{ id: string }>();
  const [org, setOrg] = useState<Organization | null>(null);
  const [customers, setCustomers] = useState<Customer[]>([]);
  const [loading, setLoading] = useState(true);

  const [vReg, setVReg] = useState('');
  const [vCap, setVCap] = useState('');
  const [vSubmitting, setVSubmitting] = useState(false);

  const [dCustomerId, setDCustomerId] = useState('');
  const [dStock, setDStock] = useState('');
  const [dQty, setDQty] = useState('');
  const [dMsg, setDMsg] = useState<{ text: string; ok: boolean } | null>(null);
  const [dSubmitting, setDSubmitting] = useState(false);

  const load = () =>
    Promise.all([getOrg(id!), listCustomers()])
      .then(([orgRes, custRes]) => {
        setOrg(orgRes.data ?? null);
        setCustomers(custRes.data ?? []);
      })
      .finally(() => setLoading(false));

  useEffect(() => { load(); }, [id]);

  const handleAddVehicle = async (e: React.FormEvent) => {
    e.preventDefault();
    setVSubmitting(true);
    try { await addVehicle(id!, vReg, Number(vCap)); setVReg(''); setVCap(''); load(); }
    finally { setVSubmitting(false); }
  };

  const handleDeleteVehicle = async (reg: string) => {
    if (!confirm(`Remove vehicle ${reg}?`)) return;
    await deleteVehicle(reg);
    load();
  };

  const handleDispatch = async (e: React.FormEvent) => {
    e.preventDefault();
    setDSubmitting(true); setDMsg(null);
    try {
      await dispatchStock(id!, dCustomerId, dStock, Number(dQty));
      setDMsg({ text: 'Dispatch successful! Stock is on its way.', ok: true });
      setDStock(''); setDQty('');
      load();
    } catch {
      setDMsg({ text: 'Dispatch failed. Check stock quantity and customer location.', ok: false });
    } finally { setDSubmitting(false); }
  };

  if (loading) {
    return (
      <div className="page">
        <div className="skeleton" style={{ width: 200, height: 28, marginBottom: 8 }} />
        <div className="skeleton" style={{ width: 300, height: 16, marginBottom: 28 }} />
        <div className="skeleton" style={{ width: '100%', height: 300, borderRadius: 12 }} />
      </div>
    );
  }

  if (!org) {
    return (
      <div className="page">
        <div className="empty-state">
          <div className="empty-state-icon"><IconBuilding size={28} /></div>
          <h3>Organization not found</h3>
          <p>This organization may have been deleted.</p>
          <Link to="/orgs" className="btn btn-primary">Back to Organizations</Link>
        </div>
      </div>
    );
  }

  const mapPins: MapPin[] = [];
  if (org.location)
    mapPins.push({ lat: org.location.latitude, lng: org.location.longitude, label: org.name, detail: org.address });
  org.vehicles.forEach(v => {
    if (v.location)
      mapPins.push({ lat: v.location.latitude, lng: v.location.longitude, label: v.registration_number, detail: `${v.capacity} MT` });
  });

  return (
    <div className="page">
      {/* Breadcrumb */}
      <nav style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 13, color: 'var(--text-3)', marginBottom: 18 }}>
        <Link to="/orgs" style={{ color: 'var(--text-3)', textDecoration: 'none' }}>Organizations</Link>
        <IconChevron size={12} />
        <span style={{ color: 'var(--text-1)' }}>{org.name}</span>
      </nav>

      {/* Page header */}
      <div className="page-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <div style={{ width: 48, height: 48, borderRadius: 12, background: 'var(--blue-bg)', color: 'var(--blue)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
            <IconBuilding size={22} />
          </div>
          <div>
            <h1 style={{ margin: 0 }}>{org.name}</h1>
            <p style={{ margin: 0, marginTop: 2, display: 'flex', alignItems: 'center', gap: 5 }}>
              <IconPin size={12} />
              {org.address}
            </p>
          </div>
        </div>
        <div style={{ display: 'flex', gap: 10 }}>
          <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '8px 14px', fontSize: 13, textAlign: 'center' }}>
            <div style={{ fontWeight: 700, fontSize: 18, color: 'var(--blue)' }}>{org.vehicles.length}</div>
            <div className="muted">Vehicles</div>
          </div>
          <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '8px 14px', fontSize: 13, textAlign: 'center' }}>
            <div style={{ fontWeight: 700, fontSize: 18, color: 'var(--purple)' }}>{org.stock.length}</div>
            <div className="muted">Stock items</div>
          </div>
        </div>
      </div>

      {/* Map */}
      {mapPins.length > 0 && (
        <div className="section-card" style={{ marginBottom: 24 }}>
          <div className="section-card-header">
            <span className="section-card-title"><IconPin size={15} />Location Map</span>
            <span className="badge tag-blue">{mapPins.length} pins</span>
          </div>
          <LocationMap pins={mapPins} height="320px" />
        </div>
      )}

      <div className="detail-grid">
        {/* Vehicles */}
        <div className="section-card" style={{ gridColumn: '1 / -1' }}>
          <div className="section-card-header">
            <span className="section-card-title"><IconTruck size={15} />Fleet Vehicles</span>
            <span className="badge">{org.vehicles.length}</span>
          </div>

          {org.vehicles.length === 0 ? (
            <div className="empty-state" style={{ padding: '28px 20px' }}>
              <div className="empty-state-icon"><IconTruck size={22} /></div>
              <h3>No vehicles</h3>
              <p>Add vehicles to this organization below.</p>
            </div>
          ) : (
            <div className="table-wrap">
              <table>
                <thead>
                  <tr><th>Registration</th><th>Capacity</th><th>Coordinates</th><th>Last Seen</th><th></th></tr>
                </thead>
                <tbody>
                  {org.vehicles.map(v => (
                    <tr key={v.registration_number}>
                      <td>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                          <div style={{ width: 28, height: 28, borderRadius: 6, background: 'var(--green-bg)', color: 'var(--green)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
                            <IconTruck size={13} />
                          </div>
                          <span className="entity-name">{v.registration_number}</span>
                        </div>
                      </td>
                      <td><span className="badge tag-blue">{v.capacity} MT</span></td>
                      <td className="coord-cell">
                        {v.location
                          ? <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}><IconPin size={11} />{v.location.latitude.toFixed(4)}, {v.location.longitude.toFixed(4)}</span>
                          : <span className="muted">Not tracked</span>}
                      </td>
                      <td className="muted">{v.location ? new Date(v.location.timestamp * 1000).toLocaleString() : '—'}</td>
                      <td>
                        <button className="btn btn-danger btn-sm" onClick={() => handleDeleteVehicle(v.registration_number)}>
                          <IconTrash size={12} />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {/* Add vehicle inline */}
          <div style={{ padding: '16px 20px', borderTop: '1px solid var(--border)', background: 'var(--bg)' }}>
            <p style={{ fontSize: 12, fontWeight: 600, color: 'var(--text-3)', textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 12 }}>Add Vehicle</p>
            <form onSubmit={handleAddVehicle} style={{ display: 'flex', gap: 8, alignItems: 'flex-end', flexWrap: 'wrap' }}>
              <div className="field" style={{ flex: 1, minWidth: 160, marginBottom: 0 }}>
                <label htmlFor="v-reg">Registration Number</label>
                <input id="v-reg" placeholder="e.g. MH12AB1234" value={vReg} onChange={e => setVReg(e.target.value)} required />
              </div>
              <div className="field" style={{ flex: '0 0 140px', marginBottom: 0 }}>
                <label htmlFor="v-cap">Capacity (MT)</label>
                <input id="v-cap" type="number" placeholder="e.g. 10" value={vCap} onChange={e => setVCap(e.target.value)} required />
              </div>
              <button className="btn btn-primary" type="submit" disabled={vSubmitting} style={{ alignSelf: 'flex-end' }}>
                <IconPlus size={14} />{vSubmitting ? 'Adding…' : 'Add Vehicle'}
              </button>
            </form>
          </div>
        </div>

        {/* Stock inventory */}
        <div className="section-card">
          <div className="section-card-header">
            <span className="section-card-title"><IconPackage size={15} />Stock Inventory</span>
            <span className="badge">{org.stock.length} items</span>
          </div>

          {org.stock.length === 0 ? (
            <div className="empty-state" style={{ padding: '28px 20px' }}>
              <div className="empty-state-icon"><IconPackage size={22} /></div>
              <h3>No stock</h3>
              <p>Stock is added via the API when goods arrive at this organization.</p>
            </div>
          ) : (
            <div className="table-wrap">
              <table>
                <thead>
                  <tr><th>Description</th><th>Quantity</th><th>Volume</th></tr>
                </thead>
                <tbody>
                  {org.stock.map(s => (
                    <tr key={s.description}>
                      <td>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                          <div style={{ width: 26, height: 26, borderRadius: 6, background: 'var(--purple-bg)', color: 'var(--purple)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
                            <IconPackage size={12} />
                          </div>
                          <span className="entity-name">{s.description}</span>
                        </div>
                      </td>
                      <td><span style={{ fontWeight: 700, color: 'var(--text-1)' }}>{s.quantity}</span> <span className="muted">units</span></td>
                      <td><span className="badge tag-blue">{s.volume_in_size}</span></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>

        {/* Dispatch form */}
        <div className="section-card">
          <div className="section-card-header">
            <span className="section-card-title"><IconDispatch size={15} />Dispatch Stock</span>
          </div>
          <div style={{ padding: '20px' }}>
            <form onSubmit={handleDispatch}>
              <div className="field">
                <label htmlFor="d-customer">Customer</label>
                <select id="d-customer" value={dCustomerId} onChange={e => setDCustomerId(e.target.value)} required>
                  <option value="">Select a customer…</option>
                  {customers.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
                </select>
              </div>
              <div className="field">
                <label htmlFor="d-stock">Stock Description</label>
                <input id="d-stock" placeholder="What is being dispatched?" value={dStock} onChange={e => setDStock(e.target.value)} required />
              </div>
              <div className="field">
                <label htmlFor="d-qty">Quantity</label>
                <input id="d-qty" type="number" placeholder="Units to dispatch" value={dQty} onChange={e => setDQty(e.target.value)} required />
              </div>

              {dMsg && (
                <div style={{ padding: '10px 14px', borderRadius: 8, marginBottom: 14, fontSize: 13, background: dMsg.ok ? 'var(--green-bg)' : 'var(--red-bg)', color: dMsg.ok ? 'var(--green)' : 'var(--red)', display: 'flex', alignItems: 'center', gap: 8 }}>
                  {dMsg.ok && <IconCheck size={14} />}
                  {dMsg.text}
                </div>
              )}

              <button className="btn btn-primary" type="submit" disabled={dSubmitting} style={{ width: '100%', justifyContent: 'center' }}>
                <IconDispatch size={14} />{dSubmitting ? 'Dispatching…' : 'Dispatch Stock'}
              </button>
            </form>
          </div>
        </div>
      </div>
    </div>
  );
}
