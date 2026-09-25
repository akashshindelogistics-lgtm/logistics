import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import Trips from './Trips';
import * as tripsApi from '../api/trips';
import * as customersApi from '../api/customers';
import * as vehiclesApi from '../api/vehicles';
import * as vendorsApi from '../api/vendors';
import type { Trip } from '../types';

vi.mock('../api/trips');
vi.mock('../api/customers');
vi.mock('../api/vehicles');
vi.mock('../api/vendors');
vi.mock('../api/auth', () => ({ getOrgId: () => 'org1' }));
vi.mock('../components/LocationMap', () => ({
  default: ({ pins }: { pins: unknown[] }) => <div data-testid="map">{pins.length} pins</div>,
}));

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
        line_items: [{ stock_description: 'Cement', quantity: 10, volume_in_size: 1, category: 'Building Materials' }],
        status: 'PENDING', dispatched_at: 1, status_history: [], proof_of_delivery: null,
        trip_id: 'trip-1', stop_sequence: 1 },
      { id: 'd2', org_id: 'org1', customer_id: 'c2', vehicle_registration_number: 'MH12 AB 1234',
        line_items: [{ stock_description: 'Cement', quantity: 5, volume_in_size: 1, category: 'Building Materials' }],
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
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(ok([]));
    vi.mocked(vendorsApi.listVendors).mockResolvedValue(ok([]));
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
    ], false, undefined);
    await waitFor(() => expect(screen.getByText('MH12 AB 1234')).toBeInTheDocument());
  });

  it('passes optimizeRoute through when the "Optimize stop order" checkbox is checked', async () => {
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
    await user.click(screen.getByLabelText(/optimize stop order/i));
    await user.click(screen.getByRole('button', { name: /^plan trip$/i }));

    expect(tripsApi.createTrip).toHaveBeenCalledWith('org1', [
      { customer_id: 'c1', line_items: [{ stock_description: 'Cement', requested_quantity: 10 }] },
      { customer_id: 'c2', line_items: [{ stock_description: 'Cement', requested_quantity: 5 }] },
    ], true, undefined);
  });

  it('shows a route map with the vehicle and every located stop when toggled on', async () => {
    const user = userEvent.setup();
    vi.mocked(tripsApi.listOrgTrips).mockResolvedValue(ok([trip()]));
    vi.mocked(customersApi.listCustomers).mockResolvedValue(
      ok([
        { ...customers[0], location: { latitude: 19.0, longitude: 72.8, timestamp: 1_700_000_000 } },
        customers[1], // no location on file
      ]) as never,
    );
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(
      ok([
        { registration_number: 'MH12 AB 1234', capacity: 10, unit: 'MetricTon',
          location: { latitude: 19.05, longitude: 72.85, timestamp: 1_700_000_500 } },
      ]),
    );
    render(<Trips />, { wrapper: MemoryRouter });
    await screen.findByText('MH12 AB 1234');

    expect(screen.queryByTestId('map')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /route map/i }));

    // One pin for the vehicle, one for the single located stop (c2 has none).
    expect(screen.getByTestId('map')).toHaveTextContent('2 pins');

    await user.click(screen.getByRole('button', { name: /hide map/i }));
    expect(screen.queryByTestId('map')).not.toBeInTheDocument();
  });

  it('shows a fallback message when no vehicle or stop has a location on file', async () => {
    const user = userEvent.setup();
    vi.mocked(tripsApi.listOrgTrips).mockResolvedValue(ok([trip()]));
    render(<Trips />, { wrapper: MemoryRouter });
    await screen.findByText('MH12 AB 1234');

    await user.click(screen.getByRole('button', { name: /route map/i }));
    expect(screen.getByText(/no location data yet/i)).toBeInTheDocument();
    expect(screen.queryByTestId('map')).not.toBeInTheDocument();
  });

  it('plans a trip on a truck hired from a vendor', async () => {
    const user = userEvent.setup();
    vi.mocked(vendorsApi.listVendors).mockResolvedValue(ok([
      { id: 'v1', org_id: 'org1', name: 'Sharma Roadlines', contact_person: null, phone: '1', gstin: null, notes: null, is_active: true },
    ]));
    vi.mocked(tripsApi.listOrgTrips).mockResolvedValue(ok([]));
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
    await user.selectOptions(screen.getByLabelText('Vehicle source'), 'v1');
    await user.click(screen.getByRole('button', { name: /^plan trip$/i }));

    expect(tripsApi.createTrip).toHaveBeenCalledWith('org1', expect.any(Array), false, 'v1');
  });

  it('shows a hired trip that is still waiting for its truck', async () => {
    vi.mocked(tripsApi.listOrgTrips).mockResolvedValue(ok([
      trip({ vehicle_registration_number: null, vehicle_source: 'HIRED', hire_id: 'h1' }),
    ]));
    render(<Trips />, { wrapper: MemoryRouter });
    expect(await screen.findByText('Awaiting hired vehicle')).toBeInTheDocument();
    expect(screen.getByText('Hired')).toBeInTheDocument();
  });
});
