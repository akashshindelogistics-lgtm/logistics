import { useEffect, useState } from 'react';
import { listOrgUsers, createOrgUser, updateOrgUser, deleteOrgUser } from '../api/users';
import { getOrgId, isAdmin } from '../api/auth';
import { IconUsers, IconPlus, IconX, IconTrash } from '../components/Icons';
import { ORG_ROLES, ROLE_LABELS, type OrgRole, type OrgUser } from '../types';
import './page.css';

const roleBadgeClass: Record<OrgRole, string> = {
  ADMIN: 'tag-purple',
  DISPATCHER: 'tag-blue',
  WAREHOUSE_STAFF: 'tag-amber',
};

export default function Team() {
  const [users, setUsers] = useState<OrgUser[]>([]);
  const [loading, setLoading] = useState(true);
  const [showForm, setShowForm] = useState(false);
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [role, setRole] = useState<OrgRole>('DISPATCHER');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState('');

  const admin = isAdmin();
  const load = () => {
    const orgId = getOrgId();
    if (!orgId) { setLoading(false); return; }
    listOrgUsers(orgId).then(r => setUsers(r.data ?? [])).catch(() => { /* ignore */ }).finally(() => setLoading(false));
  };
  useEffect(() => {
    if (admin) load();
    else setLoading(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    const orgId = getOrgId();
    if (!orgId) return;
    setSubmitting(true);
    setError('');
    try {
      await createOrgUser(orgId, { name, email, password, role });
      setName(''); setEmail(''); setPassword(''); setRole('DISPATCHER'); setShowForm(false);
      load();
    } catch (err) {
      const msg = (err as { response?: { data?: { message?: string } } }).response?.data?.message;
      setError(msg || 'Could not add the team member.');
    } finally {
      setSubmitting(false);
    }
  };

  const changeRole = async (u: OrgUser, newRole: OrgRole) => {
    await updateOrgUser(u.id, { name: u.name, role: newRole, is_active: u.is_active });
    load();
  };
  const toggleActive = async (u: OrgUser) => {
    await updateOrgUser(u.id, { name: u.name, role: u.role, is_active: !u.is_active });
    load();
  };
  const remove = async (u: OrgUser) => {
    if (!window.confirm(`Remove ${u.name} from the team?`)) return;
    await deleteOrgUser(u.id);
    load();
  };

  if (!admin) {
    return (
      <div className="page">
        <div className="empty-state">
          <div className="empty-state-icon"><IconUsers size={26} /></div>
          <h3>Admins only</h3>
          <p>Only an organization admin can manage team members.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page-header">
        <div className="page-title-group">
          <h1>Team</h1>
          <p>People who can sign in to this organization, and what they can do</p>
        </div>
        <button className="btn btn-primary" onClick={() => setShowForm(!showForm)}>
          {showForm ? <><IconX size={14} />Cancel</> : <><IconPlus size={14} />Add Member</>}
        </button>
      </div>

      {showForm && (
        <div className="form-panel">
          <h2>Add Team Member</h2>
          <form onSubmit={handleCreate}>
            <div className="field">
              <label htmlFor="u-name">Name</label>
              <input id="u-name" value={name} onChange={e => setName(e.target.value)} required />
            </div>
            <div className="field">
              <label htmlFor="u-email">Email</label>
              <input id="u-email" type="email" placeholder="person@example.com" value={email} onChange={e => setEmail(e.target.value)} required />
            </div>
            <div className="field">
              <label htmlFor="u-password">Temporary password</label>
              <input id="u-password" type="text" placeholder="At least 8 characters" value={password} onChange={e => setPassword(e.target.value)} required minLength={8} />
            </div>
            <div className="field">
              <label htmlFor="u-role">Role</label>
              <select id="u-role" value={role} onChange={e => setRole(e.target.value as OrgRole)}>
                {ORG_ROLES.map(r => <option key={r} value={r}>{ROLE_LABELS[r]}</option>)}
              </select>
            </div>
            {error && <div className="errortxt" style={{ marginBottom: 12 }}>{error}</div>}
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="btn btn-primary" type="submit" disabled={submitting}>
                {submitting ? 'Adding…' : 'Add Member'}
              </button>
              <button className="btn btn-ghost" type="button" onClick={() => setShowForm(false)}>Cancel</button>
            </div>
          </form>
        </div>
      )}

      <div className="table-card">
        <div className="table-toolbar">
          <span className="table-toolbar-title">Team Members</span>
          {!loading && <span className="badge">{users.length}</span>}
        </div>
        {loading ? (
          <div style={{ padding: 20 }}><div className="skeleton" style={{ height: 20 }} /></div>
        ) : users.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon"><IconUsers size={26} /></div>
            <h3>No team members yet</h3>
            <p>The organization sign-in is the admin. Add members so others can log in with their own role.</p>
          </div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr><th>Name</th><th>Email</th><th>Role</th><th>Status</th><th></th></tr>
              </thead>
              <tbody>
                {users.map(u => (
                  <tr key={u.id}>
                    <td className="entity-name">{u.name}</td>
                    <td className="muted">{u.email}</td>
                    <td>
                      <select
                        aria-label={`Role for ${u.name}`}
                        value={u.role}
                        onChange={e => changeRole(u, e.target.value as OrgRole)}
                        className={`badge ${roleBadgeClass[u.role]}`}
                        style={{ border: 'none', cursor: 'pointer' }}
                      >
                        {ORG_ROLES.map(r => <option key={r} value={r}>{ROLE_LABELS[r]}</option>)}
                      </select>
                    </td>
                    <td>
                      <button
                        className={`badge ${u.is_active ? 'tag-green' : ''}`}
                        onClick={() => toggleActive(u)}
                        style={{ cursor: 'pointer', border: 'none' }}
                      >
                        {u.is_active ? 'Active' : 'Inactive'}
                      </button>
                    </td>
                    <td style={{ textAlign: 'right' }}>
                      <button className="btn btn-danger btn-sm" onClick={() => remove(u)} aria-label={`Remove ${u.name}`}>
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
