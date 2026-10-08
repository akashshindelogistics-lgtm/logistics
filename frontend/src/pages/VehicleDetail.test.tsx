import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import VehicleDetail from './VehicleDetail';
import * as vehiclesApi from '../api/vehicles';
import * as driversApi from '../api/drivers';
import * as dispatchesApi from '../api/dispatches';

vi.mock('react-router-dom', async importOriginal => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return { ...actual, useParams: () => ({ reg: 'MH01AB1234' }) };
});
vi.mock('../api/vehicles');
vi.mock('../api/drivers');
vi.mock('../api/dispatches');

const ok = <T,>(data: T) => ({ success: true, message: '', data });

describe('VehicleDetail page', () => {
  beforeEach(() => vi.resetAllMocks());

  it('shows a not-found state when the vehicle is not in the org list', async () => {
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(ok([]));
    vi.mocked(driversApi.listDrivers).mockResolvedValue(ok([]));
    render(<VehicleDetail />, { wrapper: MemoryRouter });
    expect(await screen.findByText(/vehicle not found/i)).toBeInTheDocument();
  });

  it('prefills the form and saves an edited capacity and unit', async () => {
    const user = userEvent.setup();
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(
      ok([{ registration_number: 'MH01AB1234', capacity: 10, unit: 'MetricTon' }]),
    );
    vi.mocked(driversApi.listDrivers).mockResolvedValue(ok([]));
    vi.mocked(vehiclesApi.updateVehicle).mockResolvedValue(
      ok({ registration_number: 'MH01AB1234', capacity: 25, unit: 'Box' }),
    );

    render(<VehicleDetail />, { wrapper: MemoryRouter });

    const capacity = await screen.findByLabelText(/capacity/i);
    expect(capacity).toHaveValue(10);

    await user.clear(capacity);
    await user.type(capacity, '25');
    await user.selectOptions(screen.getByLabelText(/unit/i), 'Box');
    expect(screen.getByLabelText(/^type$/i)).toHaveValue('Truck');
    await user.selectOptions(screen.getByLabelText(/^type$/i), 'Pickup');
    await user.click(screen.getByRole('button', { name: /^save$/i }));

    expect(vehiclesApi.updateVehicle).toHaveBeenCalledWith('MH01AB1234', 25, 'Box', 'Pickup');
    await waitFor(() => expect(screen.getByText(/vehicle updated/i)).toBeInTheDocument());
  });

  it('resolves the assigned driver name', async () => {
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(
      ok([{ registration_number: 'MH01AB1234', capacity: 10, unit: 'MetricTon', assigned_driver_id: 'd1' }]),
    );
    vi.mocked(driversApi.listDrivers).mockResolvedValue(
      ok([{ id: 'd1', org_id: 'o1', name: 'Ravi Kumar', license_number: 'L', phone: 'p', is_active: true }]),
    );
    render(<VehicleDetail />, { wrapper: MemoryRouter });
    expect(await screen.findByText('Ravi Kumar')).toBeInTheDocument();
  });

  it('shows the GPS tracker push URL built from the vehicle key', async () => {
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(
      ok([{ registration_number: 'MH01AB1234', capacity: 10, unit: 'MetricTon', tracker_key: 'key-abc-123' }]),
    );
    vi.mocked(driversApi.listDrivers).mockResolvedValue(ok([]));
    render(<VehicleDetail />, { wrapper: MemoryRouter });

    const field = await screen.findByLabelText(/tracker push url/i);
    expect(field).toHaveValue('/api/track/key-abc-123');
  });

  it('regenerates the tracker key and shows the new URL', async () => {
    const user = userEvent.setup();
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(
      ok([{ registration_number: 'MH01AB1234', capacity: 10, unit: 'MetricTon', tracker_key: 'old-key' }]),
    );
    vi.mocked(driversApi.listDrivers).mockResolvedValue(ok([]));
    vi.mocked(vehiclesApi.rotateTrackerKey).mockResolvedValue(
      ok({ registration_number: 'MH01AB1234', capacity: 10, unit: 'MetricTon', tracker_key: 'fresh-key' }),
    );

    render(<VehicleDetail />, { wrapper: MemoryRouter });
    await screen.findByLabelText(/tracker push url/i);

    await user.click(screen.getByRole('button', { name: /regenerate key/i }));

    expect(vehiclesApi.rotateTrackerKey).toHaveBeenCalledWith('MH01AB1234');
    await waitFor(() =>
      expect(screen.getByLabelText(/tracker push url/i)).toHaveValue('/api/track/fresh-key'),
    );
  });

  it('does not show the tracker panel when the vehicle has no key', async () => {
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(
      ok([{ registration_number: 'MH01AB1234', capacity: 10, unit: 'MetricTon' }]),
    );
    vi.mocked(driversApi.listDrivers).mockResolvedValue(ok([]));
    render(<VehicleDetail />, { wrapper: MemoryRouter });
    await screen.findByLabelText(/capacity/i);
    expect(screen.queryByLabelText(/tracker push url/i)).not.toBeInTheDocument();
  });

  it('shows the vehicle as in transit from its dispatches and previews a newly picked type', async () => {
    const user = userEvent.setup();
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(
      ok([{ registration_number: 'MH01AB1234', capacity: 10, unit: 'MetricTon', vehicle_type: 'Tipper' }]),
    );
    vi.mocked(driversApi.listDrivers).mockResolvedValue(ok([]));
    vi.mocked(dispatchesApi.listDispatches).mockResolvedValue(
      ok([
        { vehicle_registration_number: 'OTHER', status: 'DELIVERED' },
        { vehicle_registration_number: 'MH01AB1234', status: 'IN_TRANSIT' },
      ] as never),
    );
    vi.mocked(vehiclesApi.listVehicleMaintenance).mockResolvedValue(ok([]));

    render(<VehicleDetail />, { wrapper: MemoryRouter });

    expect(await screen.findByAltText('Tipper illustration')).toBeInTheDocument();
    await waitFor(() => expect(screen.getByTestId('vehicle-activity')).toHaveTextContent('In transit'));

    await user.selectOptions(screen.getByLabelText(/^type$/i), 'Tanker');
    expect(screen.getByAltText('Tanker illustration')).toBeInTheDocument();
  });

  it('still renders, as available, when the activity lookups fail', async () => {
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(
      ok([{ registration_number: 'MH01AB1234', capacity: 10, unit: 'MetricTon' }]),
    );
    vi.mocked(driversApi.listDrivers).mockResolvedValue(ok([]));
    vi.mocked(dispatchesApi.listDispatches).mockRejectedValue(new Error('boom'));
    vi.mocked(vehiclesApi.listVehicleMaintenance).mockRejectedValue(new Error('boom'));

    render(<VehicleDetail />, { wrapper: MemoryRouter });

    expect(await screen.findByTestId('vehicle-activity')).toHaveTextContent('Available');
    expect(screen.getByAltText('Truck illustration')).toBeInTheDocument();
  });
});
