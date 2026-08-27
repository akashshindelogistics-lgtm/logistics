import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { listAuthOrgs, login, storeAuth, isLoggedIn, type OrgSummary } from '../api/auth';
import { IconBuilding, IconTruck } from '../components/Icons';
import './Login.css';

export default function Login() {
  const navigate = useNavigate();
  const [orgs, setOrgs] = useState<OrgSummary[]>([]);
  const [orgId, setOrgId] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [orgsLoading, setOrgsLoading] = useState(true);

  useEffect(() => {
    if (isLoggedIn()) { navigate('/', { replace: true }); return; }
    listAuthOrgs()
      .then(r => setOrgs(r.data.data ?? []))
      .finally(() => setOrgsLoading(false));
  }, [navigate]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!orgId) { setError('Please select your organization.'); return; }
    setError('');
    setLoading(true);
    try {
      const r = await login(orgId, password);
      const data = r.data.data;
      if (r.data.success && data) {
        storeAuth(data);
        navigate('/', { replace: true });
      } else {
        setError('Login failed. Please try again.');
      }
    } catch {
      setError('Invalid organization or password.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="login-root">
      <div className="login-left">
        <div className="login-brand">
          <div className="login-brand-icon">
            <IconTruck size={32} />
          </div>
          <h1>LogiTrack</h1>
          <p>Organization Logistics Platform</p>
        </div>
        <ul className="login-features">
          <li><span className="feat-icon">📦</span> Track stock across your organization</li>
          <li><span className="feat-icon">🚛</span> Manage and locate your fleet</li>
          <li><span className="feat-icon">📍</span> Dispatch to customers by distance</li>
          <li><span className="feat-icon">📊</span> Live dispatch order history</li>
        </ul>
      </div>

      <div className="login-right">
        <div className="login-card">
          <div className="login-card-header">
            <div className="login-card-icon"><IconBuilding size={22} /></div>
            <div>
              <h2>Sign in to your organization</h2>
              <p>Select your organization and enter your password</p>
            </div>
          </div>

          <form onSubmit={handleSubmit} className="login-form">
            <div className="login-field">
              <label>Organization</label>
              {orgsLoading ? (
                <div className="skeleton" style={{ height: 40, borderRadius: 8 }} />
              ) : orgs.length === 0 ? (
                <div className="login-no-orgs">
                  No organizations found.{' '}
                  <a href="/orgs" style={{ color: 'var(--brand)' }}>Create one first.</a>
                </div>
              ) : (
                <select value={orgId} onChange={e => setOrgId(e.target.value)} required>
                  <option value="">Select your organization…</option>
                  {orgs.map(o => (
                    <option key={o.id} value={o.id}>{o.name}</option>
                  ))}
                </select>
              )}
            </div>

            <div className="login-field">
              <label>Password</label>
              <input
                type="password"
                placeholder="Enter your password"
                value={password}
                onChange={e => setPassword(e.target.value)}
                required
                autoComplete="current-password"
              />
            </div>

            {error && <div className="login-error">{error}</div>}

            <button className="login-btn" type="submit" disabled={loading || orgsLoading || orgs.length === 0}>
              {loading ? 'Signing in…' : 'Sign in'}
            </button>
          </form>

          <p className="login-hint">
            New organization?{' '}
            <a href="/register" style={{ color: 'var(--brand)', fontWeight: 600 }}>Register here</a>
          </p>
        </div>
      </div>
    </div>
  );
}
