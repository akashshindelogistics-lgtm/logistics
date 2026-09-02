import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import Customers from './Customers';
import * as customersApi from '../api/customers';
import type { Customer } from '../types';

vi.mock('../api/customers');
vi.mock('../api/auth', () => ({ getOrgId: () => 'org1' }));
vi.mock('../components/LocationMap', () => ({
  default: ({ pins }: { pins: unknown[] }) => <div data-testid="map">{pins.length} pins</div>,
}));

const ok = <T,>(data: T) => ({ success: true, message: '', data });

function customer(overrides: Partial<Customer> = {}): Customer {
  return { id: 'c1', org_id: 'org1', name: 'TechHub Stores', address: '5 Market St', ...overrides };
}

describe('Customers page', () => {
  beforeEach(() => vi.resetAllMocks());

  it('shows the empty state when there are no customers', async () => {
    vi.mocked(customersApi.listCustomers).mockResolvedValue(ok([]));
    render(<Customers />);
    expect(await screen.findByText(/no customers yet/i)).toBeInTheDocument();
  });

  it('renders each customer with its address and a "not set" location', async () => {
    vi.mocked(customersApi.listCustomers).mockResolvedValue(
      ok([customer(), customer({ id: 'c2', name: 'Acme Retail', address: '9 High St' })]),
    );
    render(<Customers />);

    expect(await screen.findByText('TechHub Stores')).toBeInTheDocument();
    expect(screen.getByText('Acme Retail')).toBeInTheDocument();
    expect(screen.getAllByText(/not set/i)).toHaveLength(2);
  });

  it('plots only customers that have a location on the map', async () => {
    vi.mocked(customersApi.listCustomers).mockResolvedValue(
      ok([
        customer({ location: { latitude: 19, longitude: 72, timestamp: 0 } }),
        customer({ id: 'c2', name: 'No Geo' }),
      ]),
    );
    render(<Customers />);
    expect(await screen.findByTestId('map')).toHaveTextContent('1 pins');
  });

  it('creates a customer from the form and reloads the list', async () => {
    const user = userEvent.setup();
    vi.mocked(customersApi.listCustomers)
      .mockResolvedValueOnce(ok([]))
      .mockResolvedValueOnce(ok([customer({ name: 'Fresh Co' })]));
    vi.mocked(customersApi.createCustomer).mockResolvedValue(ok(customer({ name: 'Fresh Co' })));

    render(<Customers />);
    await screen.findByText(/no customers yet/i);

    await user.click(screen.getByRole('button', { name: /new customer/i }));
    await user.type(screen.getByLabelText(/customer name/i), 'Fresh Co');
    await user.type(screen.getByLabelText(/address/i), '1 New Rd');
    await user.click(screen.getByRole('button', { name: /^create customer$/i }));

    expect(customersApi.createCustomer).toHaveBeenCalledWith('org1', 'Fresh Co', '1 New Rd');
    await waitFor(() => expect(screen.getByText('Fresh Co')).toBeInTheDocument());
  });

  it('deletes a customer after confirmation and reloads the list', async () => {
    const user = userEvent.setup();
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    vi.mocked(customersApi.listCustomers)
      .mockResolvedValueOnce(ok([customer({ name: 'Doomed Co' })]))
      .mockResolvedValueOnce(ok([]));
    vi.mocked(customersApi.deleteCustomer).mockResolvedValue(ok(null));

    render(<Customers />);
    await screen.findByText('Doomed Co');

    await user.click(screen.getByRole('button', { name: /delete doomed co/i }));

    expect(customersApi.deleteCustomer).toHaveBeenCalledWith('c1');
    await waitFor(() => expect(screen.queryByText('Doomed Co')).not.toBeInTheDocument());
  });

  it('toggles the create form closed again from the header button', async () => {
    const user = userEvent.setup();
    vi.mocked(customersApi.listCustomers).mockResolvedValue(ok([]));
    render(<Customers />);
    await screen.findByText(/no customers yet/i);

    await user.click(screen.getByRole('button', { name: /new customer/i }));
    expect(screen.getByRole('heading', { name: /create customer/i })).toBeInTheDocument();

    // Both the header toggle and the in-form button read "Cancel"; the header one is first.
    await user.click(screen.getAllByRole('button', { name: /cancel/i })[0]);
    expect(screen.queryByRole('heading', { name: /create customer/i })).not.toBeInTheDocument();
  });

  it('shows the running customer count in the toolbar badge', async () => {
    vi.mocked(customersApi.listCustomers).mockResolvedValue(ok([customer(), customer({ id: 'c2' })]));
    render(<Customers />);
    const toolbar = (await screen.findByText('All Customers')).parentElement!;
    expect(within(toolbar).getByText('2')).toBeInTheDocument();
  });
});
