import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { listDrivers, updateDriver } from '../api/drivers';
import { IconUsers, IconChevron, IconCheck } from '../components/Icons';
import type { Driver } from '../types';
import './page.css';

export default function DriverDetail() {
  const { id } = useParams<{ id: string }>();

  const [driver, setDriver] = useState<Driver | null>(null);
  const [loading, setLoading] = useState(true);
  const [name, setName] = useState('');
  const [license, setLicense] = useState('');
  const [phone, setPhone] = useState('');
  const [isActive, setIsActive] = useState(true);
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<{ text: string; ok: boolean } | null>(null);

  useEffect(() => {
    listDrivers()
      .then(res => {
        const d = (res.data ?? []).find(x => x.id === id) ?? null;
        setDriver(d);
        if (d) {
          setName(d.name);
          setLicense(d.license_number);
          setPhone(d.phone);
          setIsActive(d.is_active);
        }
      })
      .finally(() => setLoading(false));
  }, [id]);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setMsg(null);
    try {
      const res = await updateDriver(id!, name, license, phone, isActive);
      setDriver(res.data ?? driver);
      setMsg({ text: 'Driver updated.', ok: true });
    } catch (err) {
      const apiMsg = (err as { response?: { data?: { message?: string } } }).response?.data?.message;
      setMsg({ text: apiMsg ? `Update failed: ${apiMsg}` : 'Update failed.', ok: false });
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="page">
        <div className="skeleton" style={{ width: 220, height: 28, marginBottom: 8 }} />
        <div className="skeleton" style={{ width: '100%', height: 240, borderRadius: 12 }} />
      </div>
    );
  }

  if (!driver) {
    return (
      <div className="page">
        <div className="empty-state">
          <div className="empty-state-icon"><IconUsers size={28} /></div>
          <h3>Driver not found</h3>
          <p>This driver may have been removed, or belongs to another organization.</p>
          <Link to="/orgs" className="btn btn-primary">Back to Organizations</Link>
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <nav style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 13, color: 'var(--text-3)', marginBottom: 18 }}>
        <Link to={`/orgs/${driver.org_id}`} style={{ color: 'var(--text-3)', textDecoration: 'none' }}>Organization</Link>
        <IconChevron size={12} />
        <span style={{ color: 'var(--text-1)' }}>{driver.name}</span>
      </nav>

      <div className="page-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <div style={{ width: 48, height: 48, borderRadius: 12, background: 'var(--amber-bg)', color: 'var(--amber)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
            <IconUsers size={22} />
          </div>
          <div className="page-title-group">
            <h1>{driver.name}</h1>
            <p>Edit this driver's details</p>
          </div>
        </div>
      </div>

      <div className="form-panel" style={{ maxWidth: 460 }}>
        <h2>Driver Details</h2>
        {msg && (
          <div className={msg.ok ? 'successtxt' : 'errortxt'} style={{ marginBottom: 12, display: 'flex', alignItems: 'center', gap: 6 }}>
            {msg.ok && <IconCheck size={14} />}{msg.text}
          </div>
        )}
        <form onSubmit={handleSave}>
          <div className="field">
            <label htmlFor="d-name">Name</label>
            <input id="d-name" value={name} onChange={e => setName(e.target.value)} required />
          </div>
          <div className="field">
            <label htmlFor="d-license">Licence Number</label>
            <input id="d-license" value={license} onChange={e => setLicense(e.target.value)} required />
          </div>
          <div className="field">
            <label htmlFor="d-phone">Phone</label>
            <input id="d-phone" value={phone} onChange={e => setPhone(e.target.value)} required />
          </div>
          <div className="field">
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer' }}>
              <input type="checkbox" checked={isActive} onChange={e => setIsActive(e.target.checked)} />
              Active (available to run a trip)
            </label>
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <button className="btn btn-primary" type="submit" disabled={saving}>
              {saving ? 'Saving…' : 'Save'}
            </button>
            <Link className="btn btn-ghost" to={`/orgs/${driver.org_id}`}>Cancel</Link>
          </div>
        </form>
      </div>
    </div>
  );
}
