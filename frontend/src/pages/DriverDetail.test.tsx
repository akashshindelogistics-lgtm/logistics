import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import DriverDetail from './DriverDetail';
import * as driversApi from '../api/drivers';
import type { Driver } from '../types';

const route = vi.hoisted(() => ({ id: 'd1' }));
vi.mock('react-router-dom', async importOriginal => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return { ...actual, useParams: () => ({ id: route.id }) };
});
vi.mock('../api/drivers');

const ok = <T,>(data: T) => ({ success: true, message: '', data });
const driver = (o: Partial<Driver> = {}): Driver => ({
  id: 'd1', org_id: 'o1', name: 'Ravi Kumar', license_number: 'DL-123', phone: '+91 90000', is_active: true, ...o,
});

describe('DriverDetail page', () => {
  beforeEach(() => vi.resetAllMocks());

  it('shows a not-found state for an unknown driver id', async () => {
    vi.mocked(driversApi.listDrivers).mockResolvedValue(ok([]));
    render(<DriverDetail />, { wrapper: MemoryRouter });
    expect(await screen.findByText(/driver not found/i)).toBeInTheDocument();
  });

  it('prefills and saves edited driver fields including the active flag', async () => {
    const user = userEvent.setup();
    vi.mocked(driversApi.listDrivers).mockResolvedValue(ok([driver()]));
    vi.mocked(driversApi.updateDriver).mockResolvedValue(ok(driver({ name: 'Ravi K', is_active: false })));

    render(<DriverDetail />, { wrapper: MemoryRouter });

    const name = await screen.findByLabelText(/name/i);
    expect(name).toHaveValue('Ravi Kumar');
    await user.clear(name);
    await user.type(name, 'Ravi K');
    await user.click(screen.getByRole('checkbox'));
    await user.click(screen.getByRole('button', { name: /^save$/i }));

    expect(driversApi.updateDriver).toHaveBeenCalledWith('d1', 'Ravi K', 'DL-123', '+91 90000', false);
    await waitFor(() => expect(screen.getByText(/driver updated/i)).toBeInTheDocument());
  });

  it('keeps edits when a stale load from a discarded effect lands late', async () => {
    const user = userEvent.setup();
    // In the browser, StrictMode runs the load effect twice and the first
    // response can land after the form is showing. Reproduce that with a
    // route change: the load for the previous id is held back until the
    // user has started typing, then lands.
    let resolveStale!: (v: ReturnType<typeof ok<Driver[]>>) => void;
    vi.mocked(driversApi.listDrivers)
      .mockReturnValueOnce(new Promise(r => { resolveStale = r; }) as never)
      .mockResolvedValueOnce(ok([driver()]));

    route.id = 'd0';
    const { rerender } = render(<DriverDetail />, { wrapper: MemoryRouter });
    route.id = 'd1';
    rerender(<DriverDetail />);

    const license = await screen.findByLabelText(/licence number/i);
    expect(license).toHaveValue('DL-123');
    await user.clear(license);
    await user.type(license, 'DL-NEW-123');

    await act(async () => resolveStale(ok([driver({ id: 'd0', license_number: 'DL-STALE' })])));

    expect(license).toHaveValue('DL-NEW-123');
  });
});
