import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { getOrg, dispatchStock } from '../api/orgs';
import { addVehicle, deleteVehicle } from '../api/vehicles';
import { listCustomers } from '../api/customers';
import type { Organization, Customer } from '../types';
import LocationMap, { type MapPin } from '../components/LocationMap';
import './page.css';

export default function OrganizationDetail() {
  const { id } = useParams<{ id: string }>();
  const [org, setOrg] = useState<Organization | null>(null);
  const [customers, setCustomers] = useState<Customer[]>([]);
  const [loading, setLoading] = useState(true);

  // Vehicle form
  const [vReg, setVReg] = useState('');
  const [vCap, setVCap] = useState('');

  // Dispatch form
  const [dCustomerId, setDCustomerId] = useState('');
  const [dStock, setDStock] = useState('');
  const [dQty, setDQty] = useState('');
  const [dMsg, setDMsg] = useState('');

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
    await addVehicle(id!, vReg, Number(vCap));
    setVReg(''); setVCap('');
    load();
  };

  const handleDeleteVehicle = async (reg: string) => {
    if (!confirm(`Remove vehicle ${reg}?`)) return;
    await deleteVehicle(reg);
    load();
  };

  const handleDispatch = async (e: React.FormEvent) => {
    e.preventDefault();
    try {
      await dispatchStock(id!, dCustomerId, dStock, Number(dQty));
      setDMsg('Dispatch successful!');
      setDStock(''); setDQty('');
      load();
    } catch {
      setDMsg('Dispatch failed. Check stock and customer location.');
    }
  };

  if (loading) return <div className="page"><p className="muted">Loading...</p></div>;
  if (!org) return <div className="page"><p className="error">Organization not found.</p></div>;

  const mapPins: MapPin[] = [];
  if (org.location)
    mapPins.push({ lat: org.location.latitude, lng: org.location.longitude, label: org.name, detail: org.address });
  org.vehicles.forEach(v => {
    if (v.location)
      mapPins.push({ lat: v.location.latitude, lng: v.location.longitude, label: v.registration_number, detail: `${v.capacity} MT` });
  });

  return (
    <div className="page">
      <h1>{org.name}</h1>
      <p className="muted">{org.address}</p>

      {mapPins.length > 0 && (
        <div style={{ marginBottom: '2rem' }}>
          <LocationMap pins={mapPins} height="350px" />
        </div>
      )}

      <div className="detail-grid">
        {/* Vehicles section */}
        <section className="card">
          <h2>Vehicles</h2>
          {org.vehicles.length === 0 ? <p className="muted">No vehicles.</p> : (
            <table>
              <thead><tr><th>Reg No.</th><th>Capacity</th><th>Location</th><th></th></tr></thead>
              <tbody>
                {org.vehicles.map(v => (
                  <tr key={v.registration_number}>
                    <td>{v.registration_number}</td>
                    <td>{v.capacity} MT</td>
                    <td>{v.location ? `${v.location.latitude.toFixed(3)}, ${v.location.longitude.toFixed(3)}` : '—'}</td>
                    <td><button className="btn-danger-sm" onClick={() => handleDeleteVehicle(v.registration_number)}>Remove</button></td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <form className="inline-form" onSubmit={handleAddVehicle}>
            <input placeholder="Reg number" value={vReg} onChange={e => setVReg(e.target.value)} required />
            <input placeholder="Capacity (MT)" type="number" value={vCap} onChange={e => setVCap(e.target.value)} required />
            <button className="btn-primary" type="submit">Add Vehicle</button>
          </form>
        </section>

        {/* Stock section */}
        <section className="card">
          <h2>Stock Inventory</h2>
          {org.stock.length === 0 ? <p className="muted">No stock.</p> : (
            <table>
              <thead><tr><th>Description</th><th>Quantity</th><th>Volume</th></tr></thead>
              <tbody>
                {org.stock.map(s => (
                  <tr key={s.description}>
                    <td>{s.description}</td>
                    <td>{s.quantity}</td>
                    <td>{s.volume_in_size}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </section>

        {/* Dispatch section */}
        <section className="card">
          <h2>Dispatch Stock</h2>
          <form onSubmit={handleDispatch}>
            <label>Customer
              <select value={dCustomerId} onChange={e => setDCustomerId(e.target.value)} required>
                <option value="">Select customer</option>
                {customers.map(c => <option key={c.id} value={c.id}>{c.name}</option>)}
              </select>
            </label>
            <label>Stock Description
              <input value={dStock} onChange={e => setDStock(e.target.value)} required />
            </label>
            <label>Quantity
              <input type="number" value={dQty} onChange={e => setDQty(e.target.value)} required />
            </label>
            {dMsg && <p className={dMsg.includes('failed') ? 'error' : 'success-msg'}>{dMsg}</p>}
            <button className="btn-primary" type="submit">Dispatch</button>
          </form>
        </section>
      </div>
    </div>
  );
}
