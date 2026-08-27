import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { listOrgs, createOrg, deleteOrg } from '../api/orgs';
import type { Organization } from '../types';
import './page.css';

export default function Organizations() {
  const [orgs, setOrgs] = useState<Organization[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [name, setName] = useState('');
  const [address, setAddress] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState('');

  const load = () =>
    listOrgs()
      .then(r => setOrgs(r.data ?? []))
      .finally(() => setLoading(false));

  useEffect(() => { load(); }, []);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    setError('');
    try {
      await createOrg(name, address);
      setName('');
      setAddress('');
      setShowForm(false);
      load();
    } catch {
      setError('Failed to create organization.');
    } finally {
      setSubmitting(false);
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm('Delete this organization?')) return;
    await deleteOrg(id);
    setOrgs(prev => prev.filter(o => o.id !== id));
  };

  return (
    <div className="page">
      <div className="page-header">
        <h1>Organizations</h1>
        <button className="btn-primary" onClick={() => setShowForm(!showForm)}>
          {showForm ? 'Cancel' : '+ New Organization'}
        </button>
      </div>

      {showForm && (
        <form className="card form-card" onSubmit={handleCreate}>
          <h2>New Organization</h2>
          <label>Name
            <input value={name} onChange={e => setName(e.target.value)} required />
          </label>
          <label>Address
            <input value={address} onChange={e => setAddress(e.target.value)} required />
          </label>
          {error && <p className="error">{error}</p>}
          <button className="btn-primary" disabled={submitting}>
            {submitting ? 'Creating...' : 'Create'}
          </button>
        </form>
      )}

      {loading ? (
        <p className="muted">Loading...</p>
      ) : orgs.length === 0 ? (
        <p className="muted">No organizations yet.</p>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Name</th>
                <th>Address</th>
                <th>Vehicles</th>
                <th>Location</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {orgs.map(org => (
                <tr key={org.id}>
                  <td><Link to={`/orgs/${org.id}`} className="link">{org.name}</Link></td>
                  <td>{org.address}</td>
                  <td>{org.vehicles.length}</td>
                  <td>{org.location ? `${org.location.latitude.toFixed(4)}, ${org.location.longitude.toFixed(4)}` : '—'}</td>
                  <td>
                    <button className="btn-danger-sm" onClick={() => handleDelete(org.id)}>Delete</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
