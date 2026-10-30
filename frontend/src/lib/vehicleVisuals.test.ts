import { describe, it, expect } from 'vitest';
import { VEHICLE_TYPES } from '../types';
import { ACTIVITY_STYLE, VEHICLE_VISUALS, vehicleActivity, vehicleVisual } from './vehicleVisuals';

describe('vehicleVisual', () => {
  it('has a model and icon for every vehicle type', () => {
    for (const t of VEHICLE_TYPES) {
      const v = VEHICLE_VISUALS[t];
      expect(v.type).toBe(t);
      expect(v.modelUrl).toMatch(/models\/vehicles\/[a-z-]+\.glb$/);
      expect(v.icon).toBeTruthy();
    }
  });

  it('builds the tanker from the flatbed model plus a tank', () => {
    expect(VEHICLE_VISUALS.Tanker.tank).toBe(true);
    expect(VEHICLE_VISUALS.Tanker.modelUrl).toBe(VEHICLE_VISUALS.Trailer.modelUrl);
    expect(VEHICLE_TYPES.filter(t => VEHICLE_VISUALS[t].tank)).toEqual(['Tanker']);
  });

  it('falls back to Truck for a missing or unknown type', () => {
    expect(vehicleVisual(undefined).type).toBe('Truck');
    expect(vehicleVisual(null).type).toBe('Truck');
    expect(vehicleVisual('Spaceship' as never).type).toBe('Truck');
    expect(vehicleVisual('Tipper').type).toBe('Tipper');
  });
});

describe('vehicleActivity', () => {
  const d = (vehicle_registration_number: string | null, status: string) =>
    ({ vehicle_registration_number, status }) as never;

  it('is idle with nothing going on', () => {
    expect(vehicleActivity('A', [])).toBe('idle');
  });

  it('ignores other vehicles and finished dispatches', () => {
    expect(vehicleActivity('A', [d('B', 'IN_TRANSIT'), d('A', 'DELIVERED'), d('A', 'CANCELLED'), d(null, 'AWAITING_VEHICLE')]))
      .toBe('idle');
  });

  it('is on a trip while a dispatch holds the vehicle, and in transit once it is moving', () => {
    expect(vehicleActivity('A', [d('A', 'LOADED')])).toBe('on-trip');
    expect(vehicleActivity('A', [d('A', 'PENDING'), d('A', 'IN_TRANSIT')])).toBe('in-transit');
  });

  it('flags overdue service only when the vehicle is not on the road', () => {
    expect(vehicleActivity('A', [], [{ status: 'DueSoon' }])).toBe('idle');
    expect(vehicleActivity('A', [], [{ status: 'Overdue' }])).toBe('service-due');
    expect(vehicleActivity('A', [d('A', 'CONFIRMED')], [{ status: 'Overdue' }])).toBe('on-trip');
  });

  it('only spins the wheels for a vehicle in transit', () => {
    expect(Object.entries(ACTIVITY_STYLE).filter(([, s]) => s.moving).map(([k]) => k)).toEqual(['in-transit']);
  });
});
