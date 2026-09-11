import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import Trips from './Trips';
import * as tripsApi from '../api/trips';
import * as customersApi from '../api/customers';
import type { Trip } from '../types';

vi.mock('../api/trips');
vi.mock('../api/customers');
vi.mock('../api/auth', () => ({ getOrgId: () => 'org1' }));

const ok = <T,>(data: T) => ({ success: true, message: '', data });

const customers = [
  { id: 'c1', org_id: 'org1', name: 'North Store', address: 'a' },
  { id: 'c2', org_id: 'org1', name: 'South Store', address: 'b' },
];

function trip(overrides: Partial<Trip> = {}): Trip {
  return {
    id: 'trip-1', org_id: 'org1', vehicle_registration_number: 'MH12 AB 1234',
    created_at: 1_700_000_000, status: 'PLANNED',
    stops: [
      { id: 'd1', org_id: 'org1', customer_id: 'c1', vehicle_registration_number: 'MH12 AB 1234',
        line_items: [{ stock_description: 'Cement', quantity: 10, volume_in_size: 1 }],
        status: 'PENDING', dispatched_at: 1, status_history: [], proof_of_delivery: null,
        trip_id: 'trip-1', stop_sequence: 1 },
      { id: 'd2', org_id: 'org1', customer_id: 'c2', vehicle_registration_number: 'MH12 AB 1234',
        line_items: [{ stock_description: 'Cement', quantity: 5, volume_in_size: 1 }],
        status: 'PENDING', dispatched_at: 1, status_history: [], proof_of_delivery: null,
        trip_id: 'trip-1', stop_sequence: 2 },
    ],
    ...overrides,
  };
}

describe('Trips page', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(customersApi.listCustomers).mockResolvedValue(ok(customers) as never);
  });

  it('shows an empty state when there are no trips', async () => {
    vi.mocked(tripsApi.listOrgTrips).mockResolvedValue(ok([]));
    render(<Trips />, { wrapper: MemoryRouter });
    expect(await screen.findByText(/no trips yet/i)).toBeInTheDocument();
  });

  it('lists a trip with its vehicle, status and ordered stops', async () => {
    vi.mocked(tripsApi.listOrgTrips).mockResolvedValue(ok([trip()]));
    render(<Trips />, { wrapper: MemoryRouter });

    expect(await screen.findByText('MH12 AB 1234')).toBeInTheDocument();
    expect(screen.getByText('PLANNED')).toBeInTheDocument();
    const rows = screen.getAllByRole('row').filter(r => within(r).queryByText(/Store/));
    expect(within(rows[0]).getByText('North Store')).toBeInTheDocument();
    expect(within(rows[1]).getByText('South Store')).toBeInTheDocument();
  });

  it('rejects a trip with fewer than two complete stops', async () => {
    const user = userEvent.setup();
    vi.mocked(tripsApi.listOrgTrips).mockResolvedValue(ok([]));
    render(<Trips />, { wrapper: MemoryRouter });
    await screen.findByText(/no trips yet/i);

    await user.click(screen.getByRole('button', { name: /plan a trip/i }));
    await user.selectOptions(screen.getByLabelText(/stop 1 — customer/i), 'c1');
    await user.type(screen.getAllByLabelText(/stock item/i)[0], 'Cement');
    await user.type(screen.getAllByLabelText(/quantity/i)[0], '10');
    await user.click(screen.getByRole('button', { name: /^plan trip$/i }));

    expect(await screen.findByText(/at least two complete stops/i)).toBeInTheDocument();
    expect(tripsApi.createTrip).not.toHaveBeenCalled();
  });

  it('creates a trip from two filled stops', async () => {
    const user = userEvent.setup();
    vi.mocked(tripsApi.listOrgTrips).mockResolvedValueOnce(ok([])).mockResolvedValueOnce(ok([trip()]));
    vi.mocked(tripsApi.createTrip).mockResolvedValue(ok(trip()));
    render(<Trips />, { wrapper: MemoryRouter });
    await screen.findByText(/no trips yet/i);

    await user.click(screen.getByRole('button', { name: /plan a trip/i }));
    await user.selectOptions(screen.getByLabelText(/stop 1 — customer/i), 'c1');
    await user.type(screen.getAllByLabelText(/stock item/i)[0], 'Cement');
    await user.type(screen.getAllByLabelText(/quantity/i)[0], '10');
    await user.selectOptions(screen.getByLabelText(/stop 2 — customer/i), 'c2');
    await user.type(screen.getAllByLabelText(/stock item/i)[1], 'Cement');
    await user.type(screen.getAllByLabelText(/quantity/i)[1], '5');
    await user.click(screen.getByRole('button', { name: /^plan trip$/i }));

    expect(tripsApi.createTrip).toHaveBeenCalledWith('org1', [
      { customer_id: 'c1', line_items: [{ stock_description: 'Cement', requested_quantity: 10 }] },
      { customer_id: 'c2', line_items: [{ stock_description: 'Cement', requested_quantity: 5 }] },
    ]);
    await waitFor(() => expect(screen.getByText('MH12 AB 1234')).toBeInTheDocument());
  });
});
