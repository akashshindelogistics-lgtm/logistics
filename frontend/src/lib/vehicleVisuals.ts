import type { DispatchOrder, DispatchStatus, VehicleMaintenance, VehicleType } from '../types';
import truckIcon from '../assets/3d/vehicles/truck.png';
import tipperIcon from '../assets/3d/vehicles/tipper.png';
import trailerIcon from '../assets/3d/vehicles/trailer.png';
import tempoIcon from '../assets/3d/vehicles/tempo.png';
import pickupIcon from '../assets/3d/vehicles/pickup.png';
import tankerIcon from '../assets/3d/vehicles/tanker.png';

// How each vehicle type looks: the low-poly Kenney Car Kit model (CC0) shown
// in 3D on the vehicle page, and a still render of that same model used as
// its icon in lists, on the map and as the 3D view's poster/fallback.
// Icons are regenerated with `npm run render:vehicle-icons`.

export interface VehicleVisual {
  type: VehicleType;
  label: string;
  /** One line describing the body style, for tooltips and the detail page. */
  description: string;
  /** glTF model under public/models/vehicles (its texture sits beside it). */
  modelUrl: string;
  /** Transparent PNG render of the model. */
  icon: string;
  /** The tanker has no model of its own: a tank is built onto the flatbed. */
  tank: boolean;
  /** Model nodes to leave out (e.g. the garbage truck's lifting arm). */
  hide?: string[];
}

const model = (file: string) => `${import.meta.env.BASE_URL}models/vehicles/${file}.glb`;

export const VEHICLE_VISUALS: Record<VehicleType, VehicleVisual> = {
  Truck: {
    type: 'Truck', label: 'Truck', description: 'Closed-body goods truck',
    modelUrl: model('delivery'), icon: truckIcon, tank: false,
  },
  Tipper: {
    type: 'Tipper', label: 'Tipper', description: 'Open tipper for sand, aggregate and blocks',
    modelUrl: model('garbage-truck'), icon: tipperIcon, tank: false, hide: ['arm'],
  },
  Trailer: {
    type: 'Trailer', label: 'Trailer', description: 'Flatbed trailer for long or palletised loads',
    modelUrl: model('delivery-flat'), icon: trailerIcon, tank: false,
  },
  Tempo: {
    type: 'Tempo', label: 'Tempo', description: 'Light goods carrier for short city runs',
    modelUrl: model('van'), icon: tempoIcon, tank: false,
  },
  Pickup: {
    type: 'Pickup', label: 'Pickup', description: 'Pickup for small, quick deliveries',
    modelUrl: model('truck'), icon: pickupIcon, tank: false,
  },
  Tanker: {
    type: 'Tanker', label: 'Tanker', description: 'Tanker for liquids',
    modelUrl: model('delivery-flat'), icon: tankerIcon, tank: true,
  },
};

/** The visual for a vehicle type; anything missing or unknown looks like a Truck. */
export function vehicleVisual(type?: VehicleType | null): VehicleVisual {
  return (type && VEHICLE_VISUALS[type]) || VEHICLE_VISUALS.Truck;
}

// ── What the vehicle is doing right now ──────────────────────────────────────

export type VehicleActivity = 'in-transit' | 'on-trip' | 'service-due' | 'idle';

export interface ActivityStyle {
  label: string;
  /** Hex, since it also colors the ring in the WebGL scene. */
  color: string;
  /** Spin the 3D model's wheels. */
  moving: boolean;
}

export const ACTIVITY_STYLE: Record<VehicleActivity, ActivityStyle> = {
  'in-transit': { label: 'In transit', color: '#3b82f6', moving: true },
  'on-trip': { label: 'On a trip', color: '#f59e0b', moving: false },
  'service-due': { label: 'Service overdue', color: '#ef4444', moving: false },
  idle: { label: 'Available', color: '#22c55e', moving: false },
};

/** Dispatch states that still hold the vehicle (mirrors the backend's busy check). */
const HOLDING: DispatchStatus[] = ['PENDING', 'CONFIRMED', 'LOADED', 'IN_TRANSIT'];

/**
 * Derive a vehicle's activity from the org's dispatches and its maintenance
 * schedule. A live trip outranks overdue service: the truck is on the road
 * either way, and that is what the dispatcher needs to see first.
 */
export function vehicleActivity(
  reg: string,
  dispatches: Pick<DispatchOrder, 'vehicle_registration_number' | 'status'>[],
  maintenance: Pick<VehicleMaintenance, 'status'>[] = [],
): VehicleActivity {
  const mine = dispatches.filter(d => d.vehicle_registration_number === reg);
  if (mine.some(d => d.status === 'IN_TRANSIT')) return 'in-transit';
  if (mine.some(d => HOLDING.includes(d.status))) return 'on-trip';
  if (maintenance.some(m => m.status === 'Overdue')) return 'service-due';
  return 'idle';
}
