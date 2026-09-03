import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import Dashboard from './Dashboard';
import * as orgsApi from '../api/orgs';
import * as vehiclesApi from '../api/vehicles';
import * as customersApi from '../api/customers';
import * as dispatchesApi from '../api/dispatches';
import type { DispatchOrder } from '../types';

vi.mock('../api/orgs');
vi.mock('../api/vehicles');
vi.mock('../api/customers');
vi.mock('../api/dispatches');

const ok = <T,>(data: T) => ({ success: true, message: '', data });

function order(overrides: Partial<DispatchOrder> = {}): DispatchOrder {
  return {
    id: 'order-0001-aaaa',
    org_id: 'o1',
    customer_id: 'c1',
    vehicle_registration_number: 'MH01AB1234',
    line_items: [{ stock_description: 'Cement', quantity: 20, volume_in_size: 1 }],
    status: 'PENDING',
    dispatched_at: 1_700_000_000,
    status_history: [],
    proof_of_delivery: null,
    ...overrides,
  };
}

function renderPage() {
  return render(<Dashboard />, { wrapper: MemoryRouter });
}

describe('Dashboard page', () => {
  beforeEach(() => vi.resetAllMocks());

  it('renders each stat card with the count from its endpoint', async () => {
    vi.mocked(orgsApi.listOrgs).mockResolvedValue(ok([{}, {}] as never));
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(ok([{}, {}, {}] as never));
    vi.mocked(customersApi.listCustomers).mockResolvedValue(ok([{}] as never));
    vi.mocked(dispatchesApi.listDispatches).mockResolvedValue(ok([order(), order({ id: 'b' })]));

    renderPage();

    const orgCard = (await screen.findByText('Organizations')).closest('a')!;
    expect(within(orgCard).getByText('2')).toBeInTheDocument();
    expect(within(screen.getByText('Fleet Vehicles').closest('a')!).getByText('3')).toBeInTheDocument();
    expect(within(screen.getByText('Customers').closest('a')!).getByText('1')).toBeInTheDocument();
    expect(within(screen.getByText('Dispatches').closest('a')!).getByText('2')).toBeInTheDocument();
  });

  it('shows the five most recent dispatches, newest first', async () => {
    vi.mocked(orgsApi.listOrgs).mockResolvedValue(ok([]));
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(ok([]));
    vi.mocked(customersApi.listCustomers).mockResolvedValue(ok([]));
    vi.mocked(dispatchesApi.listDispatches).mockResolvedValue(
      ok([
        order({ id: 'old-order-id', line_items: [{ stock_description: 'Older', quantity: 1, volume_in_size: 1 }], dispatched_at: 1000 }),
        order({ id: 'new-order-id', line_items: [{ stock_description: 'Newer', quantity: 1, volume_in_size: 1 }], dispatched_at: 2000 }),
      ]),
    );

    renderPage();

    const rows = await screen.findAllByRole('row');
    // rows[0] is the header; rows[1] is the newest dispatch
    expect(within(rows[1]).getByText('Newer')).toBeInTheDocument();
    expect(within(rows[2]).getByText('Older')).toBeInTheDocument();
  });

  it('caps the recent list at five rows even when more dispatches exist', async () => {
    vi.mocked(orgsApi.listOrgs).mockResolvedValue(ok([]));
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(ok([]));
    vi.mocked(customersApi.listCustomers).mockResolvedValue(ok([]));
    vi.mocked(dispatchesApi.listDispatches).mockResolvedValue(
      ok(Array.from({ length: 8 }, (_, n) => order({ id: `order-${n}`, dispatched_at: n }))),
    );

    renderPage();
    const rows = await screen.findAllByRole('row');
    expect(rows).toHaveLength(1 + 5);
  });

  it('shows the "no dispatches yet" empty state when there are none', async () => {
    vi.mocked(orgsApi.listOrgs).mockResolvedValue(ok([]));
    vi.mocked(vehiclesApi.listVehicles).mockResolvedValue(ok([]));
    vi.mocked(customersApi.listCustomers).mockResolvedValue(ok([]));
    vi.mocked(dispatchesApi.listDispatches).mockResolvedValue(ok([]));

    renderPage();
    expect(await screen.findByText(/no dispatches yet/i)).toBeInTheDocument();
  });
});
