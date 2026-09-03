import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { getGodown, updateGodown } from '../api/godowns';
import { IconBuilding, IconChevron, IconCheck } from '../components/Icons';
import type { Godown } from '../types';
import './page.css';

export default function GodownDetail() {
  const { id } = useParams<{ id: string }>();

  const [godown, setGodown] = useState<Godown | null>(null);
  const [loading, setLoading] = useState(true);
  const [name, setName] = useState('');
  const [address, setAddress] = useState('');
  const [maxCapacity, setMaxCapacity] = useState('');
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<{ text: string; ok: boolean } | null>(null);

  useEffect(() => {
    getGodown(id!)
      .then(res => {
        const g = res.data ?? null;
        setGodown(g);
        if (g) {
          setName(g.name);
          setAddress(g.address);
          setMaxCapacity(g.max_capacity != null ? String(g.max_capacity) : '');
        }
      })
      .catch(() => setGodown(null))
      .finally(() => setLoading(false));
  }, [id]);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setMsg(null);
    try {
      const cap = maxCapacity.trim() === '' ? null : Number(maxCapacity);
      const res = await updateGodown(id!, name, address, cap);
      setGodown(res.data ?? godown);
      setMsg({ text: 'Godown updated.', ok: true });
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

  if (!godown) {
    return (
      <div className="page">
        <div className="empty-state">
          <div className="empty-state-icon"><IconBuilding size={28} /></div>
          <h3>Godown not found</h3>
          <p>This godown may have been deleted, or belongs to another organization.</p>
          <Link to="/orgs" className="btn btn-primary">Back to Organizations</Link>
        </div>
      </div>
    );
  }

  const storedVolume = godown.stock.reduce((sum, s) => sum + s.volume_in_size * s.quantity, 0);

  return (
    <div className="page">
      <nav style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 13, color: 'var(--text-3)', marginBottom: 18 }}>
        <Link to={`/orgs/${godown.org_id}`} style={{ color: 'var(--text-3)', textDecoration: 'none' }}>Organization</Link>
        <IconChevron size={12} />
        <span style={{ color: 'var(--text-1)' }}>{godown.name}</span>
      </nav>

      <div className="page-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <div style={{ width: 48, height: 48, borderRadius: 12, background: 'var(--blue-bg)', color: 'var(--blue)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
            <IconBuilding size={22} />
          </div>
          <div className="page-title-group">
            <h1>{godown.name}</h1>
            <p>Edit this godown's details</p>
          </div>
        </div>
      </div>

      <div className="form-panel" style={{ maxWidth: 460 }}>
        <h2>Godown Details</h2>
        {msg && (
          <div className={msg.ok ? 'successtxt' : 'errortxt'} style={{ marginBottom: 12, display: 'flex', alignItems: 'center', gap: 6 }}>
            {msg.ok && <IconCheck size={14} />}{msg.text}
          </div>
        )}
        <form onSubmit={handleSave}>
          <div className="field">
            <label htmlFor="g-name">Name</label>
            <input id="g-name" value={name} onChange={e => setName(e.target.value)} required />
          </div>
          <div className="field">
            <label htmlFor="g-address">Address</label>
            <input id="g-address" value={address} onChange={e => setAddress(e.target.value)} required />
          </div>
          <div className="field">
            <label htmlFor="g-cap">Max capacity (total volume) — leave blank for unlimited</label>
            <input id="g-cap" type="number" min="0" value={maxCapacity} onChange={e => setMaxCapacity(e.target.value)} placeholder="Unlimited" />
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <button className="btn btn-primary" type="submit" disabled={saving}>
              {saving ? 'Saving…' : 'Save'}
            </button>
            <Link className="btn btn-ghost" to={`/orgs/${godown.org_id}`}>Cancel</Link>
          </div>
        </form>

        <div className="detail-grid" style={{ marginTop: 18, paddingTop: 14, borderTop: '1px solid var(--border)' }}>
          <div><span className="muted">Stock items</span><div>{godown.stock.length}</div></div>
          <div><span className="muted">Stored volume</span><div>{storedVolume}{godown.max_capacity != null ? ` / ${godown.max_capacity}` : ''}</div></div>
        </div>
      </div>
    </div>
  );
}
