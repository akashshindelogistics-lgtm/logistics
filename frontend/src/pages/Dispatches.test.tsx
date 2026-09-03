import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import Dispatches from './Dispatches';
import { STATUS_TAG_CLASS } from '../lib/dispatchLifecycle';
import * as dispatchesApi from '../api/dispatches';
import type { DispatchOrder } from '../types';

vi.mock('../api/dispatches');

function makeOrder(overrides: Partial<DispatchOrder> = {}): DispatchOrder {
  return {
    id: 'order-1',
    org_id: 'org-1',
    customer_id: 'cust-1',
    vehicle_registration_number: 'MH01AB1234',
    line_items: [{ stock_description: 'Cement', quantity: 50, volume_in_size: 1 }],
    status: 'PENDING',
    dispatched_at: 1_700_000_000,
    status_history: [{ status: 'PENDING', changed_at: 1_700_000_000 }],
    proof_of_delivery: null,
    ...overrides,
  };
}

describe('Dispatches page', () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it('shows an empty state when there are no orders', async () => {
    vi.mocked(dispatchesApi.listDispatches).mockResolvedValue({ success: true, message: '', data: [] });
    render(<Dispatches />);
    expect(await screen.findByText(/no dispatch orders yet/i)).toBeInTheDocument();
  });

  it('renders a PENDING order with a colored status tag and its next actions', async () => {
    vi.mocked(dispatchesApi.listDispatches).mockResolvedValue({
      success: true,
      message: '',
      data: [makeOrder()],
    });
    render(<Dispatches />);

    const tag = await screen.findByText('PENDING');
    expect(tag).toHaveClass(STATUS_TAG_CLASS.PENDING);
    expect(screen.getByRole('button', { name: 'Confirm' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument();
  });

  it('shows no lifecycle actions for a terminal status', async () => {
    vi.mocked(dispatchesApi.listDispatches).mockResolvedValue({
      success: true,
      message: '',
      data: [makeOrder({ status: 'DELIVERED', status_history: [{ status: 'DELIVERED', changed_at: 1 }] })],
    });
    render(<Dispatches />);
    await screen.findByText('DELIVERED');
    expect(screen.queryByRole('button', { name: /confirm|cancel|mark/i })).not.toBeInTheDocument();
  });

  it('advances a transition that needs no proof and updates the row from the response', async () => {
    const user = userEvent.setup();
    const order = makeOrder();
    vi.mocked(dispatchesApi.listDispatches).mockResolvedValue({ success: true, message: '', data: [order] });
    vi.mocked(dispatchesApi.updateDispatchStatus).mockResolvedValue({
      success: true,
      message: 'Dispatch status updated to CONFIRMED',
      data: {
        ...order,
        status: 'CONFIRMED',
        status_history: [...order.status_history, { status: 'CONFIRMED', changed_at: 1_700_000_100 }],
      },
    });

    render(<Dispatches />);
    await screen.findByText('PENDING');

    await user.click(screen.getByRole('button', { name: 'Confirm' }));

    await waitFor(() => expect(screen.getByText('CONFIRMED')).toBeInTheDocument());
    expect(dispatchesApi.updateDispatchStatus).toHaveBeenCalledWith('order-1', 'CONFIRMED', undefined);
    // The row now offers the next step in the lifecycle, not the old one.
    expect(screen.getByRole('button', { name: 'Mark Loaded' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Confirm' })).not.toBeInTheDocument();
  });

  it('requires receiver name and photo/signature before marking a dispatch delivered', async () => {
    const user = userEvent.setup();
    const order = makeOrder({
      status: 'IN_TRANSIT',
      status_history: [
        { status: 'PENDING', changed_at: 1 },
        { status: 'IN_TRANSIT', changed_at: 2 },
      ],
    });
    vi.mocked(dispatchesApi.listDispatches).mockResolvedValue({ success: true, message: '', data: [order] });
    vi.mocked(dispatchesApi.updateDispatchStatus).mockResolvedValue({
      success: true,
      message: 'Dispatch status updated to DELIVERED',
      data: {
        ...order,
        status: 'DELIVERED',
        proof_of_delivery: {
          receiver_name: 'Priya Sharma',
          signature_or_photo_url: 'https://example.com/sig.png',
          delivered_at: 3,
        },
      },
    });

    render(<Dispatches />);
    await screen.findByText('IN TRANSIT');

    await user.click(screen.getByRole('button', { name: 'Mark Delivered' }));

    // The API must not be called yet — proof hasn't been entered.
    expect(dispatchesApi.updateDispatchStatus).not.toHaveBeenCalled();
    const confirmBtn = screen.getByRole('button', { name: /confirm delivery/i });
    expect(confirmBtn).toBeDisabled();

    await user.type(screen.getByLabelText(/receiver name/i), 'Priya Sharma');
    expect(confirmBtn).toBeDisabled(); // still missing the photo/signature URL

    await user.type(screen.getByLabelText(/signature.*photo url/i), 'https://example.com/sig.png');
    expect(confirmBtn).toBeEnabled();

    await user.click(confirmBtn);

    expect(dispatchesApi.updateDispatchStatus).toHaveBeenCalledWith('order-1', 'DELIVERED', {
      receiver_name: 'Priya Sharma',
      signature_or_photo_url: 'https://example.com/sig.png',
    });
    await waitFor(() => expect(screen.getByText('DELIVERED')).toBeInTheDocument());
  });

  it('shows an inline error and leaves the status unchanged when the API rejects a transition', async () => {
    const user = userEvent.setup();
    const order = makeOrder();
    vi.mocked(dispatchesApi.listDispatches).mockResolvedValue({ success: true, message: '', data: [order] });
    vi.mocked(dispatchesApi.updateDispatchStatus).mockRejectedValue(new Error('Request failed with status code 400'));

    render(<Dispatches />);
    await screen.findByText('PENDING');
    await user.click(screen.getByRole('button', { name: 'Confirm' }));

    expect(await screen.findByText(/status update failed/i)).toBeInTheDocument();
    expect(screen.getByText('PENDING')).toBeInTheDocument();
  });
});
