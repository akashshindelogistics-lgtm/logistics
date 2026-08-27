import { useEffect, useState } from 'react';
import { listVehicles, deleteVehicle } from '../api/vehicles';
import type { Vehicle } from '../types';
import LocationMap, { type MapPin } from '../components/LocationMap';
import './page.css';

export default function Vehicles() {
  const [vehicles, setVehicles] = useState<Vehicle[]>([]);
  const [loading, setLoading] = useState(true);

  const load = () =>
    listVehicles()
      .then(r => setVehicles(r.data ?? []))
      .finally(() => setLoading(false));

  useEffect(() => { load(); }, []);

  const handleDelete = async (reg: string) => {
    if (!confirm(`Remove vehicle ${reg}?`)) return;
    await deleteVehicle(reg);
    setVehicles(prev => prev.filter(v => v.registration_number !== reg));
  };

  const pins: MapPin[] = vehicles
    .filter(v => v.location)
    .map(v => ({
      lat: v.location!.latitude,
      lng: v.location!.longitude,
      label: v.registration_number,
      detail: `${v.capacity} MT`,
    }));

  return (
    <div className="page">
      <div className="page-header">
        <h1>Fleet Vehicles</h1>
        <span className="badge">{vehicles.length} total</span>
      </div>

      {pins.length > 0 && (
        <div style={{ marginBottom: '1.5rem' }}>
          <LocationMap pins={pins} height="350px" />
        </div>
      )}

      {loading ? (
        <p className="muted">Loading...</p>
      ) : vehicles.length === 0 ? (
        <p className="muted">No vehicles registered. Add them from an organization's detail page.</p>
      ) : (
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                <th>Registration</th>
                <th>Capacity</th>
                <th>Latitude</th>
                <th>Longitude</th>
                <th>Last Updated</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {vehicles.map(v => (
                <tr key={v.registration_number}>
                  <td>{v.registration_number}</td>
                  <td>{v.capacity} MT</td>
                  <td>{v.location ? v.location.latitude.toFixed(4) : '—'}</td>
                  <td>{v.location ? v.location.longitude.toFixed(4) : '—'}</td>
                  <td>{v.location ? new Date(v.location.timestamp * 1000).toLocaleString() : '—'}</td>
                  <td><button className="btn-danger-sm" onClick={() => handleDelete(v.registration_number)}>Remove</button></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
