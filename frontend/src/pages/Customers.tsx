import { useEffect, useState } from 'react';
import { listCustomers, createCustomer, deleteCustomer } from '../api/customers';
import { getOrgId } from '../api/auth';
import { IconUsers, IconPlus, IconX, IconPin, IconTrash } from '../components/Icons';
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

  const load = () => listCustomers().then(r => setCustomers(r.data ?? [])).finally(() => setLoading(false));
  useEffect(() => { load(); }, []);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    const orgId = getOrgId();
    if (!orgId) return;
    setSubmitting(true);
    try { await createCustomer(orgId, name, address); setName(''); setAddress(''); setShowForm(false); load(); }
    finally { setSubmitting(false); }
  };

  const handleDelete = async (customer: Customer) => {
    if (!window.confirm(`Delete customer "${customer.name}"?`)) return;
    await deleteCustomer(customer.id);
    load();
  };

  const pins: MapPin[] = customers.filter(c => c.location).map(c => ({
    lat: c.location!.latitude, lng: c.location!.longitude,
    label: c.name, detail: c.address,
  }));

  return (
    <div className="page">
      <div className="page-header">
        <div className="page-title-group">
          <h1>Customers</h1>
          <p>Delivery recipients for your organization &mdash; not shared with other orgs</p>
        </div>
        <button className="btn btn-primary" onClick={() => setShowForm(!showForm)}>
          {showForm ? <><IconX size={14} />Cancel</> : <><IconPlus size={14} />New Customer</>}
        </button>
      </div>

      {showForm && (
        <div className="form-panel">
          <h2>Create Customer</h2>
          <form onSubmit={handleCreate}>
            <div className="field">
              <label htmlFor="cust-name">Customer Name</label>
              <input id="cust-name" placeholder="e.g. TechHub Stores" value={name} onChange={e => setName(e.target.value)} required />
            </div>
            <div className="field">
              <label htmlFor="cust-address">Address</label>
              <input id="cust-address" placeholder="Street, City, State" value={address} onChange={e => setAddress(e.target.value)} required />
            </div>
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="btn btn-primary" type="submit" disabled={submitting}>
                {submitting ? 'Creating…' : 'Create Customer'}
              </button>
              <button className="btn btn-ghost" type="button" onClick={() => setShowForm(false)}>Cancel</button>
            </div>
          </form>
        </div>
      )}

      {pins.length > 0 && (
        <div className="section-card" style={{ marginBottom: 20 }}>
          <div className="section-card-header">
            <span className="section-card-title"><IconPin size={15} />Customer Locations</span>
            <span className="badge tag-amber">{pins.length} mapped</span>
          </div>
          <LocationMap pins={pins} height="280px" />
        </div>
      )}

      <div className="table-card">
        <div className="table-toolbar">
          <span className="table-toolbar-title">All Customers</span>
          {!loading && <span className="badge">{customers.length}</span>}
        </div>

        {loading ? (
          <div style={{ padding: '20px', display: 'flex', flexDirection: 'column', gap: 14 }}>
            {[1,2,3].map(i => <div key={i} className="skeleton" style={{ height: 20 }} />)}
          </div>
        ) : customers.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon"><IconUsers size={26} /></div>
            <h3>No customers yet</h3>
            <p>Add customers to start dispatching stock to them.</p>
            <button className="btn btn-primary" onClick={() => setShowForm(true)}>
              <IconPlus size={14} />Add Customer
            </button>
          </div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr><th>Customer</th><th>Address</th><th>Location</th><th></th></tr>
              </thead>
              <tbody>
                {customers.map(c => (
                  <tr key={c.id}>
                    <td>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                        <div style={{ width: 32, height: 32, borderRadius: '50%', background: 'var(--amber-bg)', color: 'var(--amber)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0, fontWeight: 700, fontSize: 13 }}>
                          {c.name.charAt(0).toUpperCase()}
                        </div>
                        <span className="entity-name">{c.name}</span>
                      </div>
                    </td>
                    <td className="muted">{c.address}</td>
                    <td>
                      {c.location
                        ? <span className="coord-cell" style={{ display: 'flex', alignItems: 'center', gap: 4 }}><IconPin size={12} />{c.location.latitude.toFixed(4)}, {c.location.longitude.toFixed(4)}</span>
                        : <span className="muted">Not set</span>}
                    </td>
                    <td style={{ textAlign: 'right' }}>
                      <button className="btn btn-danger btn-sm" onClick={() => handleDelete(c)} aria-label={`Delete ${c.name}`}>
                        <IconTrash size={12} />
                      </button>
                    </td>
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
