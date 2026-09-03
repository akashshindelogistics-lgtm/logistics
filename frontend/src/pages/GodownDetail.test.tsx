import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import GodownDetail from './GodownDetail';
import * as godownsApi from '../api/godowns';
import type { Godown } from '../types';

vi.mock('react-router-dom', async importOriginal => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return { ...actual, useParams: () => ({ id: 'g1' }) };
});
vi.mock('../api/godowns');

const ok = <T,>(data: T) => ({ success: true, message: '', data });
const godown = (o: Partial<Godown> = {}): Godown => ({
  id: 'g1', org_id: 'o1', name: 'Warehouse A', address: '1 Dock Rd', max_capacity: null, stock: [], ...o,
});

describe('GodownDetail page', () => {
  beforeEach(() => vi.resetAllMocks());

  it('shows a not-found state when the godown fetch rejects', async () => {
    vi.mocked(godownsApi.getGodown).mockRejectedValue(new Error('404'));
    render(<GodownDetail />, { wrapper: MemoryRouter });
    expect(await screen.findByText(/godown not found/i)).toBeInTheDocument();
  });

  it('prefills and saves name, address and a cleared max capacity', async () => {
    const user = userEvent.setup();
    vi.mocked(godownsApi.getGodown).mockResolvedValue(ok(godown({ max_capacity: 5000 })));
    vi.mocked(godownsApi.updateGodown).mockResolvedValue(ok(godown({ name: 'Warehouse B' })));

    render(<GodownDetail />, { wrapper: MemoryRouter });

    const name = await screen.findByLabelText(/name/i);
    expect(name).toHaveValue('Warehouse A');
    const cap = screen.getByLabelText(/max capacity/i);
    expect(cap).toHaveValue(5000);

    await user.clear(name);
    await user.type(name, 'Warehouse B');
    await user.clear(cap);
    await user.click(screen.getByRole('button', { name: /^save$/i }));

    expect(godownsApi.updateGodown).toHaveBeenCalledWith('g1', 'Warehouse B', '1 Dock Rd', null);
    await waitFor(() => expect(screen.getByText(/godown updated/i)).toBeInTheDocument());
  });
});
