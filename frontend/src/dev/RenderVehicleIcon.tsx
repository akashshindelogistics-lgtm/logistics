import { createRoot } from 'react-dom/client';
import VehicleScene from '../components/vehicle3d/VehicleScene';
import { VEHICLE_TYPES, type VehicleType } from '../types';

// Dev-only page (scripts/render-vehicle-icons.html) that draws one vehicle
// model, still and on a transparent background, so
// scripts/render-vehicle-icons.mjs can screenshot it into an icon PNG.
// Not part of the app bundle.

declare global {
  interface Window { __vehicleReady?: boolean }
}

const requested = new URLSearchParams(location.search).get('type') as VehicleType | null;
const type: VehicleType = requested && VEHICLE_TYPES.includes(requested) ? requested : 'Truck';

createRoot(document.getElementById('root')!).render(
  <div style={{ width: 640, height: 640 }}>
    <VehicleScene type={type} interactive={false} onReady={() => { window.__vehicleReady = true; }} />
  </div>,
);
