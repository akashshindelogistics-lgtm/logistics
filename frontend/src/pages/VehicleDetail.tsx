import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { listVehicles, updateVehicle } from '../api/vehicles';
import { listDrivers } from '../api/drivers';
import { IconTruck, IconChevron, IconCheck } from '../components/Icons';
import type { Driver, Unit, Vehicle } from '../types';
import { UNITS } from '../types';
import './page.css';

export default function VehicleDetail() {
  const { reg: rawReg } = useParams<{ reg: string }>();
  const reg = decodeURIComponent(rawReg ?? '');

  const [vehicle, setVehicle] = useState<Vehicle | null>(null);
  const [drivers, setDrivers] = useState<Driver[]>([]);
  const [loading, setLoading] = useState(true);
  const [capacity, setCapacity] = useState('');
  const [unit, setUnit] = useState<Unit>('MetricTon');
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<{ text: string; ok: boolean } | null>(null);

  useEffect(() => {
    Promise.all([listVehicles(), listDrivers()])
      .then(([vRes, dRes]) => {
        const v = (vRes.data ?? []).find(x => x.registration_number === reg) ?? null;
        setVehicle(v);
        setDrivers(dRes.data ?? []);
        if (v) {
          setCapacity(String(v.capacity));
          setUnit(v.unit);
        }
      })
      .finally(() => setLoading(false));
  }, [reg]);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setMsg(null);
    try {
      const res = await updateVehicle(reg, Number(capacity), unit);
      setVehicle(res.data ?? vehicle);
      setMsg({ text: 'Vehicle updated.', ok: true });
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
        <div className="skeleton" style={{ width: '100%', height: 220, borderRadius: 12 }} />
      </div>
    );
  }

  if (!vehicle) {
    return (
      <div className="page">
        <div className="empty-state">
          <div className="empty-state-icon"><IconTruck size={28} /></div>
          <h3>Vehicle not found</h3>
          <p>This vehicle may have been removed, or belongs to another organization.</p>
          <Link to="/vehicles" className="btn btn-primary">Back to Fleet</Link>
        </div>
      </div>
    );
  }

  const assignedDriver = drivers.find(d => d.id === vehicle.assigned_driver_id);

  return (
    <div className="page">
      <nav style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 13, color: 'var(--text-3)', marginBottom: 18 }}>
        <Link to="/vehicles" style={{ color: 'var(--text-3)', textDecoration: 'none' }}>Fleet Vehicles</Link>
        <IconChevron size={12} />
        <span style={{ color: 'var(--text-1)' }}>{vehicle.registration_number}</span>
      </nav>

      <div className="page-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <div style={{ width: 48, height: 48, borderRadius: 12, background: 'var(--green-bg)', color: 'var(--green)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
            <IconTruck size={22} />
          </div>
          <div className="page-title-group">
            <h1>{vehicle.registration_number}</h1>
            <p>Edit this vehicle's details</p>
          </div>
        </div>
      </div>

      <div className="form-panel" style={{ maxWidth: 460 }}>
        <h2>Vehicle Details</h2>
        {msg && (
          <div className={msg.ok ? 'successtxt' : 'errortxt'} style={{ marginBottom: 12, display: 'flex', alignItems: 'center', gap: 6 }}>
            {msg.ok && <IconCheck size={14} />}{msg.text}
          </div>
        )}
        <form onSubmit={handleSave}>
          <div className="field">
            <label htmlFor="v-cap">Capacity</label>
            <input id="v-cap" type="number" min="1" value={capacity} onChange={e => setCapacity(e.target.value)} required />
          </div>
          <div className="field">
            <label htmlFor="v-unit">Unit</label>
            <select id="v-unit" value={unit} onChange={e => setUnit(e.target.value as Unit)}>
              {UNITS.map(u => <option key={u} value={u}>{u}</option>)}
            </select>
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <button className="btn btn-primary" type="submit" disabled={saving}>
              {saving ? 'Saving…' : 'Save'}
            </button>
            <Link className="btn btn-ghost" to="/vehicles">Cancel</Link>
          </div>
        </form>

        <div className="detail-grid" style={{ marginTop: 18, paddingTop: 14, borderTop: '1px solid var(--border)' }}>
          <div><span className="muted">Assigned driver</span><div>{assignedDriver ? assignedDriver.name : <span className="muted">None</span>}</div></div>
          <div><span className="muted">Location</span><div>{vehicle.location ? `${vehicle.location.latitude.toFixed(4)}, ${vehicle.location.longitude.toFixed(4)}` : <span className="muted">Not set</span>}</div></div>
        </div>
      </div>
    </div>
  );
}
