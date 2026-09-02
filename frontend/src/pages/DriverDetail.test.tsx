import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import DriverDetail from './DriverDetail';
import * as driversApi from '../api/drivers';
import type { Driver } from '../types';

vi.mock('react-router-dom', async importOriginal => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return { ...actual, useParams: () => ({ id: 'd1' }) };
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
});
