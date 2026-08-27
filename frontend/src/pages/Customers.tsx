import { useEffect, useState } from 'react';
import { listCustomers, createCustomer } from '../api/customers';
import type { Customer } from '../types';
import LocationMap, { type MapPin } from '../components/LocationMap';
import './page.css';

export default function Customers() {
  const [customers, setCustomers] = useState<Customer[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [name, setName] = useState('');
  const [address, setAddress] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const load = () =>
    listCustomers()
      .then(r => setCustomers(r.data ?? []))
      .finally(() => setLoading(false));

  useEffect(() => { load(); }, []);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    try {
      await createCustomer(name, address);
      setName(''); setAddress('');
      setShowForm(false);
      load();
    } finally {
      setSubmitting(false);
    }
  };

  const pins: MapPin[] = customers
    .filter(c => c.location)
    .map(c => ({
      lat: c.location!.latitude,
      lng: c.location!.longitude,
      label: c.name,
      detail: c.address,
    }));

  return (
    <div className="page">
      <div className="page-header">
        <h1>Customers</h1>
        <button className="btn-primary" onClick={() => setShowForm(!showForm)}>
          {showForm ? 'Cancel' : '+ New Customer'}
        </button>
      </div>

      {showForm && (
        <form className="card form-card" onSubmit={handleCreate}>
          <h2>New Customer</h2>
          <label>Name
            <input value={name} onChange={e => setName(e.target.value)} required />
          </label>
          <label>Address
            <input value={address} onChange={e => setAddress(e.target.value)} required />
          </label>
          <button className="btn-primary" disabled={submitting}>
            {submitting ? 'Creating...' : 'Create'}
          </button>
        </form>
      )}

      {pins.length > 0 && (
        <div style={{ marginBottom: '1.5rem' }}>
          <LocationMap pins={pins} height="300px" />
        </div>
      )}

      {loading ? (
        <p className="muted">Loading...</p>
      ) : customers.length === 0 ? (
        <p className="muted">No customers yet.</p>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Name</th>
                <th>Address</th>
                <th>Location</th>
              </tr>
            </thead>
            <tbody>
              {customers.map(c => (
                <tr key={c.id}>
                  <td>{c.name}</td>
                  <td>{c.address}</td>
                  <td>{c.location ? `${c.location.latitude.toFixed(4)}, ${c.location.longitude.toFixed(4)}` : '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
