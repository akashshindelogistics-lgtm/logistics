import { useEffect, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { listVehicles, deleteVehicle } from '../api/vehicles';
import { IconPin } from '../components/Icons';
import Icon3D from '../components/Icon3D';
import type { Vehicle } from '../types';
import { vehicleVisual } from '../lib/vehicleVisuals';
import LocationMap, { type MapPin } from '../components/LocationMap';
import './page.css';
import { useListAnimation, useLivePolling } from '../lib/motion';
import { useTransitionClick, VEHICLE_HERO } from '../lib/viewTransition';

/**
 * Type icon + registration link. Opening the vehicle morphs this icon into the
 * detail page's header icon (a view transition); the type rides along in the
 * link state so the detail page can draw that header before its data loads.
 */
function VehicleCell({ vehicle: v }: { vehicle: Vehicle }) {
  const iconRef = useRef<HTMLImageElement>(null);
  const href = `/vehicles/${encodeURIComponent(v.registration_number)}`;
  const state = { vehicleType: v.vehicle_type };
  const onClick = useTransitionClick(href, { state, morph: { name: VEHICLE_HERO, element: () => iconRef.current } });
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
      <img ref={iconRef} className="vehicle-type-icon" src={vehicleVisual(v.vehicle_type).icon} alt="" width={40} height={40} />
      <Link to={href} state={state} onClick={onClick} className="entity-name" style={{ textDecoration: 'none', color: 'var(--blue)' }}>
        {v.registration_number}
      </Link>
    </div>
  );
}

export default function Vehicles() {
  const rowsRef = useListAnimation();
  const [vehicles, setVehicles] = useState<Vehicle[]>([]);
  const [loading, setLoading] = useState(true);

  const load = () => listVehicles().then(r => setVehicles(r.data ?? [])).finally(() => setLoading(false));
  useEffect(() => { load(); }, []);
  // Re-read positions while the page is open so tracked vehicles glide on the map.
  useLivePolling(() => { listVehicles().then(r => setVehicles(r.data ?? [])).catch(() => {}); });

  const handleDelete = async (reg: string) => {
    if (!confirm(`Remove vehicle ${reg}?`)) return;
    await deleteVehicle(reg);
    setVehicles(prev => prev.filter(v => v.registration_number !== reg));
  };

  const pins: MapPin[] = vehicles.filter(v => v.location).map(v => ({
    lat: v.location!.latitude, lng: v.location!.longitude,
    id: v.registration_number,
    label: v.registration_number,
    detail: `${vehicleVisual(v.vehicle_type).label} · ${v.capacity} ${v.unit}`,
    iconUrl: vehicleVisual(v.vehicle_type).icon,
  }));

  const withLocation = vehicles.filter(v => v.location).length;

  return (
    <div className="page">
      <div className="page-header">
        <div className="page-title-group">
          <h1>Fleet Vehicles</h1>
          <p>Track and manage your vehicle fleet</p>
        </div>
        <div style={{ display: 'flex', gap: 10 }}>
          <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '8px 16px', fontSize: 13 }}>
            <strong>{vehicles.length}</strong> <span className="muted">total</span>
          </div>
          <div style={{ background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 8, padding: '8px 16px', fontSize: 13 }}>
            <strong style={{ color: 'var(--green)' }}>{withLocation}</strong> <span className="muted">tracked</span>
          </div>
        </div>
      </div>

      {pins.length > 0 && (
        <div className="section-card" style={{ marginBottom: 20 }}>
          <div className="section-card-header">
            <span className="section-card-title"><IconPin size={15} />Live Fleet Map</span>
            <span className="badge tag-green">{pins.length} vehicles on map</span>
          </div>
          <LocationMap pins={pins} height="320px" />
        </div>
      )}

      <div className="table-card">
        <div className="table-toolbar">
          <span className="table-toolbar-title">All Vehicles</span>
          {!loading && <span className="badge">{vehicles.length}</span>}
        </div>

        {loading ? (
          <div style={{ padding: '20px', display: 'flex', flexDirection: 'column', gap: 14 }}>
            {[1,2,3].map(i => <div key={i} className="skeleton" style={{ height: 20 }} />)}
          </div>
        ) : vehicles.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon empty-state-icon-3d"><Icon3D name="truck" size={48} /></div>
            <h3>No vehicles registered</h3>
            <p>Add vehicles from an organization's detail page.</p>
          </div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr><th>Registration</th><th>Type</th><th>Capacity</th><th>Latitude</th><th>Longitude</th><th>Last Updated</th><th></th></tr>
              </thead>
              <tbody ref={rowsRef}>
                {vehicles.map(v => (
                  <tr key={v.registration_number}>
                    <td>
                      <VehicleCell vehicle={v} />
                    </td>
                    <td title={vehicleVisual(v.vehicle_type).description}>{vehicleVisual(v.vehicle_type).label}</td>
                    <td><span className="badge tag-blue">{v.capacity} {v.unit}</span></td>
                    <td className="coord-cell">{v.location ? v.location.latitude.toFixed(5) : <span className="muted">—</span>}</td>
                    <td className="coord-cell">{v.location ? v.location.longitude.toFixed(5) : <span className="muted">—</span>}</td>
                    <td className="muted">{v.location ? new Date(v.location.timestamp * 1000).toLocaleString() : '—'}</td>
                    <td>
                      <button className="btn btn-danger btn-sm" onClick={() => handleDelete(v.registration_number)}>Remove</button>
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
