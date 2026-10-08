import { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import { listVehicles, updateVehicle, rotateTrackerKey, listVehicleMaintenance } from '../api/vehicles';
import { listDrivers } from '../api/drivers';
import { listDispatches } from '../api/dispatches';
import { IconChevron, IconCheck } from '../components/Icons';
import Icon3D from '../components/Icon3D';
import VehicleShowcase from '../components/vehicle3d/VehicleShowcase';
import { vehicleActivity, vehicleVisual, type VehicleActivity } from '../lib/vehicleVisuals';
import type { Driver, Unit, Vehicle, VehicleType } from '../types';
import { UNITS, VEHICLE_TYPES } from '../types';
import './page.css';

export default function VehicleDetail() {
  const { reg: rawReg } = useParams<{ reg: string }>();
  const reg = decodeURIComponent(rawReg ?? '');

  const [vehicle, setVehicle] = useState<Vehicle | null>(null);
  const [drivers, setDrivers] = useState<Driver[]>([]);
  const [loading, setLoading] = useState(true);
  const [capacity, setCapacity] = useState('');
  const [unit, setUnit] = useState<Unit>('MetricTon');
  const [vehicleType, setVehicleType] = useState<VehicleType>('Truck');
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState<{ text: string; ok: boolean } | null>(null);
  const [rotating, setRotating] = useState(false);
  const [keyMsg, setKeyMsg] = useState<string | null>(null);
  const [activity, setActivity] = useState<VehicleActivity>('idle');

  useEffect(() => {
    Promise.all([listVehicles(), listDrivers()])
      .then(([vRes, dRes]) => {
        const v = (vRes.data ?? []).find(x => x.registration_number === reg) ?? null;
        setVehicle(v);
        setDrivers(dRes.data ?? []);
        if (v) {
          setCapacity(String(v.capacity));
          setUnit(v.unit);
          setVehicleType(v.vehicle_type ?? 'Truck');
        }
      })
      .finally(() => setLoading(false));
  }, [reg]);

  // What the vehicle is doing, for the 3D view's status ring. Best effort:
  // if either list fails the vehicle just shows as available.
  useEffect(() => {
    let cancelled = false;
    const orEmpty = <T,>(load: () => Promise<{ data?: T[] | null }>) =>
      Promise.resolve().then(load).then(r => r?.data ?? []).catch(() => [] as T[]);
    Promise.all([orEmpty(listDispatches), orEmpty(() => listVehicleMaintenance(reg))]).then(([dispatches, maintenance]) => {
      if (!cancelled) setActivity(vehicleActivity(reg, dispatches, maintenance));
    });
    return () => { cancelled = true; };
  }, [reg]);

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setMsg(null);
    try {
      const res = await updateVehicle(reg, Number(capacity), unit, vehicleType);
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
          <div className="empty-state-icon empty-state-icon-3d"><Icon3D name="truck" size={48} /></div>
          <h3>Vehicle not found</h3>
          <p>This vehicle may have been removed, or belongs to another organization.</p>
          <Link to="/vehicles" className="btn btn-primary">Back to Fleet</Link>
        </div>
      </div>
    );
  }

  const assignedDriver = drivers.find(d => d.id === vehicle.assigned_driver_id);

  const handleRotateKey = async () => {
    if (!window.confirm('Issue a new tracker key? Every GPS device on this vehicle must be reconfigured with the new key.')) return;
    setRotating(true);
    setKeyMsg(null);
    try {
      const res = await rotateTrackerKey(reg);
      if (res.data) setVehicle(res.data);
      setKeyMsg('New tracker key issued.');
    } catch {
      setKeyMsg('Could not rotate the key. Try again.');
    } finally {
      setRotating(false);
    }
  };

  return (
    <div className="page">
      <nav style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 13, color: 'var(--text-3)', marginBottom: 18 }}>
        <Link to="/vehicles" style={{ color: 'var(--text-3)', textDecoration: 'none' }}>Fleet Vehicles</Link>
        <IconChevron size={12} />
        <span style={{ color: 'var(--text-1)' }}>{vehicle.registration_number}</span>
      </nav>

      <div className="page-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <img src={vehicleVisual(vehicle.vehicle_type).icon} alt="" width={56} height={56} style={{ objectFit: 'contain', flexShrink: 0 }} />
          <div className="page-title-group">
            <h1>{vehicle.registration_number}</h1>
            <p>Edit this vehicle's details</p>
          </div>
        </div>
      </div>

      <div className="vehicle-detail-grid">
      <VehicleShowcase type={vehicleType} activity={activity} />

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
          <div className="field">
            <label htmlFor="v-type">Type</label>
            <select id="v-type" value={vehicleType} onChange={e => setVehicleType(e.target.value as VehicleType)}>
              {VEHICLE_TYPES.map(t => <option key={t} value={t}>{t}</option>)}
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

      {vehicle.tracker_key && (
        <div className="form-panel" style={{ maxWidth: 460, marginTop: 18 }}>
          <h2>GPS Tracker</h2>
          <p className="muted" style={{ marginTop: -4, fontSize: 13 }}>
            A tracker device fitted to this vehicle reports its position automatically by
            POSTing <code>{'{ latitude, longitude }'}</code> to the URL below — no login, the
            key is the credential. Keep it secret; rotate it if a device is lost.
          </p>
          {keyMsg && (
            <div className="successtxt" style={{ marginBottom: 10, display: 'flex', alignItems: 'center', gap: 6 }}>
              <IconCheck size={14} />{keyMsg}
            </div>
          )}
          <div className="field">
            <label htmlFor="v-tracker">Tracker push URL</label>
            <input
              id="v-tracker"
              readOnly
              value={`/api/track/${vehicle.tracker_key}`}
              onFocus={e => e.currentTarget.select()}
              style={{ fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace', fontSize: 12 }}
            />
          </div>
          <button className="btn btn-ghost" type="button" onClick={handleRotateKey} disabled={rotating}>
            {rotating ? 'Rotating…' : 'Regenerate key'}
          </button>
        </div>
      )}
    </div>
  );
}
