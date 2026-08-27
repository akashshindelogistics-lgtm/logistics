import { useEffect, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { listOrgs, deleteOrg } from '../api/orgs';
import { getOrgId } from '../api/auth';
import { IconBuilding, IconPin, IconChevron } from '../components/Icons';
import type { Organization } from '../types';
import './page.css';

export default function Organizations() {
  const navigate = useNavigate();
  const orgId = getOrgId();
  const [orgs, setOrgs] = useState<Organization[]>([]);
  const [loading, setLoading] = useState(true);

  const load = () => listOrgs().then(r => setOrgs(r.data ?? [])).finally(() => setLoading(false));

  useEffect(() => {
    // Redirect straight to the authenticated org's detail
    if (orgId) { navigate(`/orgs/${orgId}`, { replace: true }); return; }
    load();
  }, [orgId]);

  const handleDelete = async (id: string) => {
    if (!confirm('Delete this organization and all its data?')) return;
    await deleteOrg(id);
    setOrgs(prev => prev.filter(o => o.id !== id));
  };

  return (
    <div className="page">
      <div className="page-header">
        <div className="page-title-group">
          <h1>Organizations</h1>
          <p>Your logistics network</p>
        </div>
      </div>

      <div className="table-card">
        <div className="table-toolbar">
          <span className="table-toolbar-title">All Organizations</span>
          {!loading && <span className="badge">{orgs.length}</span>}
        </div>

        {loading ? (
          <div style={{ padding: '20px' }}>
            {[1,2,3,4].map(i => (
              <div key={i} style={{ display: 'flex', gap: 12, marginBottom: 14, alignItems: 'center' }}>
                <div className="skeleton" style={{ width: 32, height: 32, borderRadius: 8 }} />
                <div style={{ flex: 1 }}>
                  <div className="skeleton" style={{ width: '40%', height: 14, marginBottom: 6 }} />
                  <div className="skeleton" style={{ width: '60%', height: 12 }} />
                </div>
              </div>
            ))}
          </div>
        ) : orgs.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon"><IconBuilding size={26} /></div>
            <h3>No organizations found</h3>
            <p>You are not associated with any organization.</p>
          </div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr><th>Organization</th><th>Address</th><th>Vehicles</th><th>Location</th><th></th></tr>
              </thead>
              <tbody>
                {orgs.map(org => (
                  <tr key={org.id}>
                    <td>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                        <div style={{ width: 32, height: 32, borderRadius: 8, background: 'var(--blue-bg)', color: 'var(--blue)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
                          <IconBuilding size={15} />
                        </div>
                        <Link to={`/orgs/${org.id}`} className="nav-link-cell entity-name">{org.name}</Link>
                      </div>
                    </td>
                    <td className="muted">{org.address}</td>
                    <td>
                      {org.vehicles.length > 0
                        ? <span className="badge">{org.vehicles.length}</span>
                        : <span className="muted">—</span>}
                    </td>
                    <td>
                      {org.location
                        ? <span className="coord-cell" style={{ display: 'flex', alignItems: 'center', gap: 4 }}><IconPin size={12} />{org.location.latitude.toFixed(4)}, {org.location.longitude.toFixed(4)}</span>
                        : <span className="muted">Not set</span>}
                    </td>
                    <td>
                      <div style={{ display: 'flex', gap: 6, justifyContent: 'flex-end' }}>
                        <Link to={`/orgs/${org.id}`} className="btn btn-ghost btn-sm">
                          Details <IconChevron size={12} />
                        </Link>
                        <button className="btn btn-danger btn-sm" onClick={() => handleDelete(org.id)}>
                          Delete
                        </button>
                      </div>
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
