import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import OrganizationDetail from './OrganizationDetail';
import * as orgsApi from '../api/orgs';
import * as vehiclesApi from '../api/vehicles';
import * as customersApi from '../api/customers';
import * as godownsApi from '../api/godowns';
import * as driversApi from '../api/drivers';
import type { Customer, Driver, Organization } from '../types';

vi.mock('react-router-dom', async importOriginal => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return { ...actual, useParams: () => ({ id: 'o1' }) };
});
vi.mock('../api/orgs');
vi.mock('../api/vehicles');
vi.mock('../api/customers');
vi.mock('../api/godowns');
vi.mock('../api/drivers');
vi.mock('../components/LocationMap', () => ({
  default: ({ pins }: { pins: unknown[] }) => <div data-testid="map">{pins.length} pins</div>,
}));

const ok = <T,>(data: T) => ({ success: true, message: '', data });

function org(overrides: Partial<Organization> = {}): Organization {
  return {
    id: 'o1',
    name: 'Express Freight',
    address: '1 Dock Rd',
    vehicles: [],
    godowns: [],
    ...overrides,
  };
}

const customer: Customer = { id: 'c1', org_id: 'o1', name: 'TechHub Stores', address: '5 Market St' };

function mockLoad(
  orgValue: Organization | null,
  customers: Customer[] = [customer],
  drivers: Driver[] = [],
) {
  vi.mocked(orgsApi.getOrg).mockResolvedValue(ok(orgValue ?? undefined));
  vi.mocked(customersApi.listCustomers).mockResolvedValue(ok(customers));
  vi.mocked(driversApi.listDrivers).mockResolvedValue(ok(drivers));
  vi.mocked(godownsApi.listStockTransfers).mockResolvedValue(ok([]));
}

function renderPage() {
  return render(<OrganizationDetail />, { wrapper: MemoryRouter });
}

describe('OrganizationDetail page', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    // Every render calls listDrivers() and listStockTransfers(); default them
    // so tests that don't care about drivers/transfers don't have to mock them.
    vi.mocked(driversApi.listDrivers).mockResolvedValue(ok([]));
    vi.mocked(godownsApi.listStockTransfers).mockResolvedValue(ok([]));
  });
  afterEach(() => vi.unstubAllGlobals());

  it('shows a not-found state when the org cannot be loaded', async () => {
    mockLoad(null);
    renderPage();
    expect(await screen.findByText(/organization not found/i)).toBeInTheDocument();
  });

  it('renders the org header with its vehicle and godown counts', async () => {
    mockLoad(
      org({
        vehicles: [{ registration_number: 'MH01AB1234', capacity: 10, unit: 'MetricTon' }],
        godowns: [
          { id: 'g1', org_id: 'o1', name: 'North Godown', address: 'Plot 5', stock: [] },
        ],
      }),
    );
    renderPage();

    expect(await screen.findByRole('heading', { name: 'Express Freight' })).toBeInTheDocument();
    expect(screen.getByText('1 Dock Rd')).toBeInTheDocument();
    expect(screen.getByText('MH01AB1234')).toBeInTheDocument();
    expect(screen.getByText('10 MT')).toBeInTheDocument();
    expect(screen.getByText('North Godown')).toBeInTheDocument();
    expect(screen.getByText('Plot 5')).toBeInTheDocument();
  });

  it('shows the "no vehicles" / "no godowns" empty states for a bare org', async () => {
    mockLoad(org());
    renderPage();
    expect(await screen.findByText(/no vehicles$/i)).toBeInTheDocument();
    expect(screen.getByText(/no godowns$/i)).toBeInTheDocument();
  });

  it('adds a vehicle with a numeric capacity and reloads the org', async () => {
    const user = userEvent.setup();
    vi.mocked(customersApi.listCustomers).mockResolvedValue(ok([customer]));
    vi.mocked(orgsApi.getOrg)
      .mockResolvedValueOnce(ok(org()))
      .mockResolvedValue(
        ok(org({ vehicles: [{ registration_number: 'MH09XY9999', capacity: 15, unit: 'MetricTon' }] })),
      );
    vi.mocked(vehiclesApi.addVehicle).mockResolvedValue(ok({} as never));

    renderPage();
    await screen.findByText(/no vehicles$/i);

    await user.type(screen.getByLabelText(/registration number/i), 'MH09XY9999');
    await user.type(screen.getByLabelText(/capacity/i), '15');
    await user.click(screen.getByRole('button', { name: /add vehicle/i }));

    expect(vehiclesApi.addVehicle).toHaveBeenCalledWith('o1', 'MH09XY9999', 15);
    await waitFor(() => expect(screen.getByText('MH09XY9999')).toBeInTheDocument());
  });

  it('confirms a successful dispatch and clears the stock/qty fields', async () => {
    const user = userEvent.setup();
    mockLoad(org());
    vi.mocked(orgsApi.dispatchStock).mockResolvedValue(ok({} as never));

    renderPage();
    await screen.findByRole('heading', { name: 'Express Freight' });

    await user.selectOptions(screen.getByLabelText(/customer/i), 'c1');
    await user.type(screen.getByLabelText(/stock description/i), 'Cement');
    await user.type(screen.getByLabelText(/quantity/i), '30');
    await user.click(screen.getByRole('button', { name: /dispatch stock/i }));

    expect(orgsApi.dispatchStock).toHaveBeenCalledWith('o1', 'c1', 'Cement', 30);
    expect(await screen.findByText(/dispatch successful/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/stock description/i)).toHaveValue('');
  });

  it('shows a failure message when the dispatch call rejects', async () => {
    const user = userEvent.setup();
    mockLoad(org());
    vi.mocked(orgsApi.dispatchStock).mockRejectedValue(new Error('insufficient stock'));

    renderPage();
    await screen.findByRole('heading', { name: 'Express Freight' });

    await user.selectOptions(screen.getByLabelText(/customer/i), 'c1');
    await user.type(screen.getByLabelText(/stock description/i), 'Cement');
    await user.type(screen.getByLabelText(/quantity/i), '30');
    await user.click(screen.getByRole('button', { name: /dispatch stock/i }));

    expect(await screen.findByText(/dispatch failed/i)).toBeInTheDocument();
  });

  it('adds stock to a specific godown via that godown\'s inline form', async () => {
    const user = userEvent.setup();
    const godown = { id: 'g1', org_id: 'o1', name: 'North Godown', address: 'Plot 5', stock: [] };
    vi.mocked(customersApi.listCustomers).mockResolvedValue(ok([customer]));
    vi.mocked(orgsApi.getOrg).mockResolvedValue(ok(org({ godowns: [godown] })));
    vi.mocked(godownsApi.addGodownStock).mockResolvedValue(ok({} as never));

    renderPage();
    await screen.findByText('North Godown');

    await user.type(screen.getByLabelText(/stock item/i), 'Cement Bags');
    await user.type(screen.getByLabelText(/stock quantity/i), '100');
    await user.type(screen.getByLabelText(/volume/i), '3');
    await user.click(screen.getByRole('button', { name: /add stock/i }));

    expect(godownsApi.addGodownStock).toHaveBeenCalledWith('g1', 'Cement Bags', 100, 3);
  });

  it('transfers stock from one godown to another via the inline form', async () => {
    const user = userEvent.setup();
    const godowns = [
      {
        id: 'g1', org_id: 'o1', name: 'North Godown', address: 'Plot 5',
        stock: [{ description: 'Cement', quantity: 100, volume_in_size: 5 }],
      },
      { id: 'g2', org_id: 'o1', name: 'South Godown', address: 'Plot 9', stock: [] },
    ];
    vi.mocked(customersApi.listCustomers).mockResolvedValue(ok([customer]));
    vi.mocked(orgsApi.getOrg).mockResolvedValue(ok(org({ godowns })));
    vi.mocked(godownsApi.transferGodownStock).mockResolvedValue(ok({} as never));

    renderPage();
    await screen.findByText('North Godown');

    await user.selectOptions(screen.getByLabelText(/transfer item/i), 'Cement');
    await user.selectOptions(screen.getByLabelText(/to godown/i), 'g2');
    await user.type(screen.getByLabelText(/transfer quantity/i), '30');
    await user.click(screen.getByRole('button', { name: /^transfer$/i }));

    expect(godownsApi.transferGodownStock).toHaveBeenCalledWith('g1', 'g2', 'Cement', 30);
    expect(await screen.findByText(/stock transferred between godowns/i)).toBeInTheDocument();
  });

  it('lists the godown-to-godown transfer history', async () => {
    mockLoad(
      org({
        godowns: [
          { id: 'g1', org_id: 'o1', name: 'North Godown', address: 'Plot 5', stock: [] },
          { id: 'g2', org_id: 'o1', name: 'South Godown', address: 'Plot 9', stock: [] },
        ],
      }),
    );
    vi.mocked(godownsApi.listStockTransfers).mockResolvedValue(
      ok([
        {
          id: 't1', org_id: 'o1', from_godown_id: 'g1', to_godown_id: 'g2',
          description: 'Cement', quantity: 40, volume_in_size: 5, transferred_at: 1_700_000_000,
        },
      ]),
    );

    renderPage();

    expect(await screen.findByText('Stock Transfers')).toBeInTheDocument();
    const row = screen.getByText('Cement').closest('tr')!;
    expect(within(row).getByText('North Godown')).toBeInTheDocument();
    expect(within(row).getByText('South Godown')).toBeInTheDocument();
    expect(within(row).getByText('40')).toBeInTheDocument();
  });

  it('deletes a vehicle only after the confirm prompt is accepted', async () => {
    const user = userEvent.setup();
    vi.stubGlobal('confirm', vi.fn(() => true));
    vi.mocked(customersApi.listCustomers).mockResolvedValue(ok([customer]));
    vi.mocked(orgsApi.getOrg)
      .mockResolvedValueOnce(
        ok(org({ vehicles: [{ registration_number: 'MH01AB1234', capacity: 10, unit: 'MetricTon' }] })),
      )
      .mockResolvedValue(ok(org()));
    vi.mocked(vehiclesApi.deleteVehicle).mockResolvedValue(ok(null));

    renderPage();
    const row = (await screen.findByText('MH01AB1234')).closest('tr')!;
    await user.click(within(row).getByRole('button'));

    expect(vehiclesApi.deleteVehicle).toHaveBeenCalledWith('MH01AB1234');
    await waitFor(() => expect(screen.queryByText('MH01AB1234')).not.toBeInTheDocument());
  });

  it('lists drivers and adds one through the inline form', async () => {
    const user = userEvent.setup();
    mockLoad(org(), [customer], [
      { id: 'd1', org_id: 'o1', name: 'Ravi Kumar', license_number: 'DL-1', phone: '111', is_active: true },
    ]);
    vi.mocked(driversApi.addDriver).mockResolvedValue(ok({} as never));

    renderPage();
    expect(await screen.findByText('Ravi Kumar')).toBeInTheDocument();

    await user.type(screen.getByLabelText('Driver Name'), 'Sunita Rao');
    await user.type(screen.getByLabelText('Licence Number'), 'MH-9');
    await user.type(screen.getByLabelText('Phone'), '+91 99999 00000');
    await user.click(screen.getByRole('button', { name: /add driver/i }));

    expect(driversApi.addDriver).toHaveBeenCalledWith('o1', 'Sunita Rao', 'MH-9', '+91 99999 00000');
  });

  it('assigns a driver to a vehicle from the fleet table', async () => {
    const user = userEvent.setup();
    mockLoad(
      org({ vehicles: [{ registration_number: 'MH01AB1234', capacity: 10, unit: 'MetricTon' }] }),
      [customer],
      [{ id: 'd1', org_id: 'o1', name: 'Ravi Kumar', license_number: 'DL-1', phone: '111', is_active: true }],
    );
    vi.mocked(driversApi.assignVehicleDriver).mockResolvedValue(ok({} as never));

    renderPage();
    const select = await screen.findByLabelText('Driver for MH01AB1234');
    await user.selectOptions(select, 'd1');

    expect(driversApi.assignVehicleDriver).toHaveBeenCalledWith('MH01AB1234', 'd1');
  });
});
