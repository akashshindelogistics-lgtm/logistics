import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import Vehicles from './Vehicles';
import * as vehiclesApi from '../api/vehicles';
import type { Vehicle } from '../types';

vi.mock('../api/vehicles');
vi.mock('../components/LocationMap', () => ({
  default: ({ pins }: { pins: unknown[] }) => <div data-testid="map">{pins.length} pins</div>,
}));

const ok = <T,>(data: T) => ({ success: true, message: '', data });

function vehicle(overrides: Partial<Vehicle> = {}): Vehicle {
  return { registration_number: 'MH01AB1234', capacity: 10, unit: 'MetricTon', ...overrides };
}

describe('Vehicles page', () => {
  beforeEach(() => vi.resetAllMocks());
  afterEach(() => vi.unstubAllGlobals());

  it('shows the empty state when the fleet is empty', async () => {
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(ok([]));
    render(<Vehicles />);
    expect(await screen.findByText(/no vehicles registered/i)).toBeInTheDocument();
  });

  it('lists vehicles and counts how many are location-tracked', async () => {
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(
      ok([
        vehicle({ location: { latitude: 19.076, longitude: 72.877, timestamp: 1_700_000_000 } }),
        vehicle({ registration_number: 'MH02CD5678' }),
      ]),
    );
    render(<Vehicles />);

    expect(await screen.findByText('MH01AB1234')).toBeInTheDocument();
    expect(screen.getByText('MH02CD5678')).toBeInTheDocument();
    expect(screen.getByText('1 vehicles on map')).toBeInTheDocument();
    // "tracked" stat tile reads "<count> tracked"
    expect(screen.getByText('tracked').parentElement).toHaveTextContent('1 tracked');
  });

  it('removes a vehicle after the user confirms the prompt', async () => {
    const user = userEvent.setup();
    vi.stubGlobal('confirm', vi.fn(() => true));
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(ok([vehicle()]));
    vi.mocked(vehiclesApi.deleteVehicle).mockResolvedValue(ok(null));

    render(<Vehicles />);
    await screen.findByText('MH01AB1234');
    await user.click(screen.getByRole('button', { name: /remove/i }));

    expect(vehiclesApi.deleteVehicle).toHaveBeenCalledWith('MH01AB1234');
    await waitFor(() => expect(screen.queryByText('MH01AB1234')).not.toBeInTheDocument());
  });

  it('does not delete when the user cancels the confirm prompt', async () => {
    const user = userEvent.setup();
    vi.stubGlobal('confirm', vi.fn(() => false));
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(ok([vehicle()]));

    render(<Vehicles />);
    await screen.findByText('MH01AB1234');
    await user.click(screen.getByRole('button', { name: /remove/i }));

    expect(vehiclesApi.deleteVehicle).not.toHaveBeenCalled();
    expect(screen.getByText('MH01AB1234')).toBeInTheDocument();
  });
});
