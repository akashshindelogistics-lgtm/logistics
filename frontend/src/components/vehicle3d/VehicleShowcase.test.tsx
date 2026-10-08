import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import VehicleShowcase from './VehicleShowcase';
import { VEHICLE_VISUALS } from '../../lib/vehicleVisuals';

// The real scene needs WebGL; jsdom has none, so the showcase must never load it.
vi.mock('./VehicleScene', () => {
  throw new Error('VehicleScene must not load without WebGL');
});

describe('VehicleShowcase', () => {
  it('shows the still render of the type as a poster when WebGL is unavailable', () => {
    render(<VehicleShowcase type="Tanker" activity="idle" />);
    const showcase = screen.getByTestId('vehicle-showcase');
    expect(showcase).toHaveAttribute('data-mode', 'poster');
    expect(screen.getByAltText('Tanker illustration')).toHaveAttribute('src', VEHICLE_VISUALS.Tanker.icon);
    expect(screen.getByText(/tanker for liquids/i)).toBeInTheDocument();
    expect(screen.queryByText(/drag to rotate/i)).not.toBeInTheDocument();
  });

  it('labels and colors the status', () => {
    render(<VehicleShowcase type="Truck" activity="in-transit" />);
    const showcase = screen.getByTestId('vehicle-showcase');
    expect(showcase).toHaveAttribute('data-activity', 'in-transit');
    expect(showcase.style.getPropertyValue('--status')).toBe('#3b82f6');
    expect(screen.getByTestId('vehicle-activity')).toHaveTextContent('In transit');
  });

  it('treats a missing type as a Truck', () => {
    render(<VehicleShowcase activity="service-due" />);
    expect(screen.getByAltText('Truck illustration')).toBeInTheDocument();
    expect(screen.getByTestId('vehicle-activity')).toHaveTextContent('Service overdue');
  });
});
