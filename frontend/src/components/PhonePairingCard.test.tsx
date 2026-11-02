import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import PhonePairingCard from './PhonePairingCard';
import * as driversApi from '../api/drivers';
import type { DevicePairing } from '../types';

vi.mock('../api/drivers');
vi.mock('../api/driverApp', () => ({ serverAddress: () => 'https://logi-api.example.org' }));

const ok = <T,>(data: T) => ({ success: true, message: '', data });
const TOKEN = '1b4e28ba-2fa1-11d2-883f-0016d3cca427';
const nowSecs = () => Math.floor(Date.now() / 1000);
const unpaired: DevicePairing = { paired: false, paired_at: null, last_seen_at: null };

describe('PhonePairingCard', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    localStorage.setItem('logi_role', 'DISPATCHER');
  });

  it('pairs an unpaired driver and shows the token once, with a QR code', async () => {
    const user = userEvent.setup();
    vi.mocked(driversApi.getDevicePairing).mockResolvedValue(ok(unpaired));
    vi.mocked(driversApi.rotateDeviceToken).mockResolvedValue(ok({ driver_id: 'd1', device_token: TOKEN }));
    render(<PhonePairingCard driverId="d1" />);

    expect(await screen.findByTestId('pairing-status')).toHaveTextContent('Not paired');
    await user.click(screen.getByRole('button', { name: 'Pair a phone' }));

    expect(driversApi.rotateDeviceToken).toHaveBeenCalledWith('d1');
    expect(await screen.findByTestId('pairing-token')).toHaveTextContent(TOKEN);
    expect(screen.getByTestId('pairing-server')).toHaveTextContent('https://logi-api.example.org');
    expect(screen.getByTestId('pairing-status')).toHaveTextContent('Paired');
    expect((await screen.findByTestId('pairing-qr')).getAttribute('src')).toMatch(/^data:image\/png;base64,/);

    await user.click(screen.getByRole('button', { name: 'Copy token' }));
    expect(await navigator.clipboard.readText()).toBe(TOKEN);

    // Done hides the token for good.
    await user.click(screen.getByRole('button', { name: 'Done' }));
    expect(screen.queryByTestId('pairing-token')).not.toBeInTheDocument();
    expect(screen.getByTestId('pairing-detail')).toHaveTextContent("the phone hasn't connected yet");
  });

  it('shows when a paired phone last reported', async () => {
    vi.mocked(driversApi.getDevicePairing).mockResolvedValue(
      ok({ paired: true, paired_at: nowSecs() - 2 * 86_400, last_seen_at: nowSecs() - 5 * 60 }));
    render(<PhonePairingCard driverId="d1" />);

    expect(await screen.findByTestId('pairing-detail')).toHaveTextContent('Paired 2 days ago · last reported 5 min ago');
  });

  it('asks before pairing a new phone over a paired one', async () => {
    const user = userEvent.setup();
    vi.mocked(driversApi.getDevicePairing).mockResolvedValue(ok({ paired: true, paired_at: nowSecs(), last_seen_at: null }));
    vi.mocked(driversApi.rotateDeviceToken).mockResolvedValue(ok({ driver_id: 'd1', device_token: TOKEN }));
    render(<PhonePairingCard driverId="d1" />);

    await user.click(await screen.findByRole('button', { name: 'Pair a new phone' }));
    expect(screen.getByTestId('pairing-confirm')).toHaveTextContent('stops reporting at once');
    expect(driversApi.rotateDeviceToken).not.toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(driversApi.rotateDeviceToken).not.toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: 'Pair a new phone' }));
    await user.click(screen.getByRole('button', { name: 'Pair a new phone' }));
    expect(await screen.findByTestId('pairing-token')).toHaveTextContent(TOKEN);
  });

  it('unpairs after confirmation', async () => {
    const user = userEvent.setup();
    vi.mocked(driversApi.getDevicePairing).mockResolvedValue(ok({ paired: true, paired_at: nowSecs(), last_seen_at: nowSecs() }));
    vi.mocked(driversApi.unpairDevice).mockResolvedValue(ok(unpaired));
    render(<PhonePairingCard driverId="d1" />);

    await user.click(await screen.findByRole('button', { name: 'Unpair' }));
    await user.click(screen.getByRole('button', { name: 'Unpair' }));

    expect(driversApi.unpairDevice).toHaveBeenCalledWith('d1');
    expect(await screen.findByTestId('pairing-status')).toHaveTextContent('Not paired');
    expect(screen.getByRole('button', { name: 'Pair a phone' })).toBeInTheDocument();
  });

  it('shows warehouse staff the status but no pairing buttons', async () => {
    localStorage.setItem('logi_role', 'WAREHOUSE_STAFF');
    vi.mocked(driversApi.getDevicePairing).mockResolvedValue(ok(unpaired));
    render(<PhonePairingCard driverId="d1" />);

    expect(await screen.findByText(/only an admin or dispatcher/i)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Pair a phone' })).not.toBeInTheDocument();
  });

  it('says when pairing fails', async () => {
    const user = userEvent.setup();
    vi.mocked(driversApi.getDevicePairing).mockResolvedValue(ok(unpaired));
    vi.mocked(driversApi.rotateDeviceToken).mockImplementation(() => Promise.reject(new Error('403')));
    render(<PhonePairingCard driverId="d1" />);

    await user.click(await screen.findByRole('button', { name: 'Pair a phone' }));
    expect(await screen.findByText("Couldn't issue a device token.")).toBeInTheDocument();
    expect(screen.queryByTestId('pairing-token')).not.toBeInTheDocument();
  });
});
