import { useState } from 'react';
import { useNavigate, Link } from 'react-router-dom';
import { createOrg } from '../api/orgs';
import { login, storeAuth } from '../api/auth';
import { IconBuilding, IconTruck } from '../components/Icons';
import './Login.css';

export default function Register() {
  const navigate = useNavigate();
  const [name, setName] = useState('');
  const [address, setAddress] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (password !== confirm) { setError('Passwords do not match.'); return; }
    if (password.length < 6) { setError('Password must be at least 6 characters.'); return; }
    setError('');
    setLoading(true);
    try {
      const r = await createOrg(name, address, password);
      if (r.data.success && r.data.data) {
        // Auto-login after registration
        const loginRes = await login(r.data.data.id, password);
        if (loginRes.data.success && loginRes.data.data) {
          storeAuth(loginRes.data.data);
          navigate(`/orgs/${r.data.data.id}`, { replace: true });
        }
      } else {
        setError('Registration failed. Please try again.');
      }
    } catch {
      setError('Failed to create organization. Please try again.');
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
              <h2>Register your organization</h2>
              <p>Create an account to start managing logistics</p>
            </div>
          </div>

          <form onSubmit={handleSubmit} className="login-form">
            <div className="login-field">
              <label htmlFor="reg-name">Organization Name</label>
              <input
                id="reg-name"
                placeholder="e.g. Express Freight Co."
                value={name}
                onChange={e => setName(e.target.value)}
                required
              />
            </div>
            <div className="login-field">
              <label htmlFor="reg-address">Address</label>
              <input
                id="reg-address"
                placeholder="Street, City, State"
                value={address}
                onChange={e => setAddress(e.target.value)}
                required
              />
            </div>
            <div className="login-field">
              <label htmlFor="reg-password">Password</label>
              <input
                id="reg-password"
                type="password"
                placeholder="Choose a secure password"
                value={password}
                onChange={e => setPassword(e.target.value)}
                required
                autoComplete="new-password"
              />
            </div>
            <div className="login-field">
              <label htmlFor="reg-confirm">Confirm Password</label>
              <input
                id="reg-confirm"
                type="password"
                placeholder="Repeat your password"
                value={confirm}
                onChange={e => setConfirm(e.target.value)}
                required
                autoComplete="new-password"
              />
            </div>

            {error && <div className="login-error">{error}</div>}

            <button className="login-btn" type="submit" disabled={loading}>
              {loading ? 'Creating account…' : 'Create Organization'}
            </button>
          </form>

          <p className="login-hint">
            Already registered?{' '}
            <Link to="/login" style={{ color: 'var(--brand)', fontWeight: 600 }}>Sign in</Link>
          </p>
        </div>
      </div>
    </div>
  );
}
