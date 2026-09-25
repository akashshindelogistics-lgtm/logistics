import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import Vendors from './Vendors';
import * as vendorsApi from '../api/vendors';
import * as authApi from '../api/auth';
import type { VehicleVendor } from '../types';

vi.mock('../api/vendors');
vi.mock('../api/auth');

const ok = <T,>(data: T) => ({ success: true, message: '', data });

function vendor(o: Partial<VehicleVendor> = {}): VehicleVendor {
  return {
    id: 'v1', org_id: 'o1', name: 'Sharma Roadlines', contact_person: 'Anil Sharma',
    phone: '+91 98200 00000', gstin: '27AAPFU0939F1ZV', notes: 'Mumbai-Pune lane', is_active: true,
    ...o,
  };
}

describe('Vendors page', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(authApi.getOrgId).mockReturnValue('o1');
    vi.mocked(authApi.getRole).mockReturnValue('DISPATCHER');
  });

  it('lists vendors with their contact details and status', async () => {
    vi.mocked(vendorsApi.listVendors).mockResolvedValue(ok([
      vendor(),
      vendor({ id: 'v2', name: 'Balaji Transport', contact_person: null, gstin: null, notes: null, is_active: false }),
    ]));
    render(<Vendors />);

    const sharma = (await screen.findByText('Sharma Roadlines')).closest('tr') as HTMLElement;
    expect(within(sharma).getByText('Anil Sharma')).toBeInTheDocument();
    expect(within(sharma).getByText('27AAPFU0939F1ZV')).toBeInTheDocument();
    expect(within(sharma).getByText('Active')).toBeInTheDocument();

    const balaji = screen.getByText('Balaji Transport').closest('tr') as HTMLElement;
    expect(within(balaji).getByText('Inactive')).toBeInTheDocument();
    expect(vendorsApi.listVendors).toHaveBeenCalledWith('o1');
  });

  it('shows an empty state when the org has no vendors', async () => {
    vi.mocked(vendorsApi.listVendors).mockResolvedValue(ok([]));
    render(<Vendors />);
    expect(await screen.findByText(/no vendors yet/i)).toBeInTheDocument();
  });

  it('adds a vendor from the form, sending blank optional fields as null', async () => {
    const u = userEvent.setup();
    vi.mocked(vendorsApi.listVendors)
      .mockResolvedValueOnce(ok([]))
      .mockResolvedValueOnce(ok([vendor({ name: 'New Carrier' })]));
    vi.mocked(vendorsApi.createVendor).mockResolvedValue(ok(vendor({ name: 'New Carrier' })));

    render(<Vendors />);
    await screen.findByText(/no vendors yet/i);

    await u.click(screen.getByRole('button', { name: /add vendor/i }));
    await u.type(screen.getByLabelText(/vendor name/i), 'New Carrier');
    await u.type(screen.getByLabelText(/phone/i), '022 1234 5678');
    await u.click(screen.getByRole('button', { name: /^add vendor$/i }));

    expect(vendorsApi.createVendor).toHaveBeenCalledWith('o1', {
      name: 'New Carrier', contact_person: null, phone: '022 1234 5678', gstin: null, notes: null,
    });
    await waitFor(() => expect(screen.getByText('New Carrier')).toBeInTheDocument());
  });

  it('shows the server message when a save is rejected', async () => {
    const u = userEvent.setup();
    vi.mocked(vendorsApi.listVendors).mockResolvedValue(ok([]));
    vi.mocked(vendorsApi.createVendor).mockRejectedValue({
      response: { data: { message: 'GSTIN must be 15 letters and digits' } },
    });

    render(<Vendors />);
    await screen.findByText(/no vendors yet/i);
    await u.click(screen.getByRole('button', { name: /add vendor/i }));
    await u.type(screen.getByLabelText(/vendor name/i), 'Bad GSTIN Co');
    await u.type(screen.getByLabelText(/phone/i), '1');
    await u.type(screen.getByLabelText(/gstin/i), 'ABC');
    await u.click(screen.getByRole('button', { name: /^add vendor$/i }));

    expect(await screen.findByText('GSTIN must be 15 letters and digits')).toBeInTheDocument();
  });

  it('edits a vendor through the pre-filled form', async () => {
    const u = userEvent.setup();
    vi.mocked(vendorsApi.listVendors).mockResolvedValue(ok([vendor()]));
    vi.mocked(vendorsApi.updateVendor).mockResolvedValue(ok(vendor({ phone: '+91 1' })));

    render(<Vendors />);
    await u.click(await screen.findByRole('button', { name: /edit sharma roadlines/i }));
    const phone = screen.getByLabelText(/phone/i);
    expect(phone).toHaveValue('+91 98200 00000');
    await u.clear(phone);
    await u.type(phone, '+91 1');
    await u.click(screen.getByRole('button', { name: /save vendor/i }));

    expect(vendorsApi.updateVendor).toHaveBeenCalledWith('v1', {
      name: 'Sharma Roadlines', contact_person: 'Anil Sharma', phone: '+91 1',
      gstin: '27AAPFU0939F1ZV', notes: 'Mumbai-Pune lane', is_active: true,
    });
  });

  it('toggles a vendor inactive from the status badge', async () => {
    const u = userEvent.setup();
    vi.mocked(vendorsApi.listVendors).mockResolvedValue(ok([vendor()]));
    vi.mocked(vendorsApi.updateVendor).mockResolvedValue(ok(vendor({ is_active: false })));

    render(<Vendors />);
    await u.click(await screen.findByRole('button', { name: /deactivate sharma roadlines/i }));
    expect(vendorsApi.updateVendor).toHaveBeenCalledWith('v1', expect.objectContaining({ is_active: false }));
  });

  it('deletes a vendor after confirmation', async () => {
    const u = userEvent.setup();
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    vi.mocked(vendorsApi.listVendors).mockResolvedValue(ok([vendor()]));
    vi.mocked(vendorsApi.deleteVendor).mockResolvedValue(ok(null));

    render(<Vendors />);
    await u.click(await screen.findByRole('button', { name: /delete sharma roadlines/i }));
    expect(vendorsApi.deleteVendor).toHaveBeenCalledWith('v1');
  });

  it('is read-only for warehouse staff', async () => {
    vi.mocked(authApi.getRole).mockReturnValue('WAREHOUSE_STAFF');
    vi.mocked(vendorsApi.listVendors).mockResolvedValue(ok([vendor()]));
    render(<Vendors />);

    await screen.findByText('Sharma Roadlines');
    expect(screen.queryByRole('button', { name: /add vendor/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /edit sharma roadlines/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /deactivate/i })).not.toBeInTheDocument();
  });
});
