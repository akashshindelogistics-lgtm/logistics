import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { listAuthOrgs, login, userLogin, storeAuth, isLoggedIn, type OrgSummary } from '../api/auth';
import { IconBuilding, IconTruck, IconUsers } from '../components/Icons';
import './Login.css';

type Mode = 'org' | 'user';

export default function Login() {
  const navigate = useNavigate();
  const [mode, setMode] = useState<Mode>('org');
  const [orgs, setOrgs] = useState<OrgSummary[]>([]);
  const [orgId, setOrgId] = useState('');
  const [email, setEmail] = useState('');
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

  const switchMode = (m: Mode) => { setMode(m); setError(''); };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (mode === 'org' && !orgId) { setError('Please select your organization.'); return; }
    setError('');
    setLoading(true);
    try {
      const r = mode === 'org' ? await login(orgId, password) : await userLogin(email, password);
      const data = r.data.data;
      if (r.data.success && data) {
        storeAuth(data);
        navigate(`/orgs/${data.org_id}`, { replace: true });
      } else {
        setError('Login failed. Please try again.');
      }
    } catch {
      setError(mode === 'org'
        ? 'Invalid credentials. Please check your password.'
        : 'Invalid email or password, or your account is inactive.');
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
            <div className="login-card-icon">{mode === 'org' ? <IconBuilding size={22} /> : <IconUsers size={22} />}</div>
            <div>
              <h2>{mode === 'org' ? 'Sign in to your organization' : 'Sign in as a team member'}</h2>
              <p>{mode === 'org'
                ? 'Select your organization and enter its password'
                : 'Enter the email and password your admin gave you'}</p>
            </div>
          </div>

          <div className="login-mode-tabs" role="tablist">
            <button type="button" role="tab" aria-selected={mode === 'org'}
              className={`login-mode-tab${mode === 'org' ? ' active' : ''}`} onClick={() => switchMode('org')}>
              Organization
            </button>
            <button type="button" role="tab" aria-selected={mode === 'user'}
              className={`login-mode-tab${mode === 'user' ? ' active' : ''}`} onClick={() => switchMode('user')}>
              Team member
            </button>
          </div>

          <form onSubmit={handleSubmit} className="login-form">
            {mode === 'org' ? (
              <div className="login-field">
                <label htmlFor="login-org">Organization</label>
                {orgsLoading ? (
                  <div className="skeleton" style={{ height: 40, borderRadius: 8 }} />
                ) : orgs.length === 0 ? (
                  <div className="login-no-orgs">
                    No organizations found.{' '}
                    <a href="/register" style={{ color: 'var(--brand)' }}>Create one first.</a>
                  </div>
                ) : (
                  <select id="login-org" value={orgId} onChange={e => setOrgId(e.target.value)} required>
                    <option value="">Select your organization…</option>
                    {orgs.map(o => (
                      <option key={o.id} value={o.id}>{o.name}</option>
                    ))}
                  </select>
                )}
              </div>
            ) : (
              <div className="login-field">
                <label htmlFor="login-email">Email</label>
                <input id="login-email" type="email" placeholder="you@example.com"
                  value={email} onChange={e => setEmail(e.target.value)} required autoComplete="username" />
              </div>
            )}

            <div className="login-field">
              <label htmlFor="login-password">Password</label>
              <input
                id="login-password"
                type="password"
                placeholder="Enter your password"
                value={password}
                onChange={e => setPassword(e.target.value)}
                required
                autoComplete="current-password"
              />
            </div>

            {error && <div className="login-error">{error}</div>}

            <button className="login-btn" type="submit"
              disabled={loading || (mode === 'org' && (orgsLoading || orgs.length === 0))}>
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
