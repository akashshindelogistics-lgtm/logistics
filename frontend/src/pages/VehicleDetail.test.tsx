import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import VehicleDetail from './VehicleDetail';
import * as vehiclesApi from '../api/vehicles';
import * as driversApi from '../api/drivers';

vi.mock('react-router-dom', async importOriginal => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return { ...actual, useParams: () => ({ reg: 'MH01AB1234' }) };
});
vi.mock('../api/vehicles');
vi.mock('../api/drivers');

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
    await user.click(screen.getByRole('button', { name: /^save$/i }));

    expect(vehiclesApi.updateVehicle).toHaveBeenCalledWith('MH01AB1234', 25, 'Box');
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
});
