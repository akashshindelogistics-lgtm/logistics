import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import Team from './Team';
import * as usersApi from '../api/users';
import * as authApi from '../api/auth';
import type { OrgUser } from '../types';

vi.mock('../api/users');
vi.mock('../api/auth');

const ok = <T,>(data: T) => ({ success: true, message: '', data });

function user(o: Partial<OrgUser> = {}): OrgUser {
  return { id: 'u1', org_id: 'o1', name: 'Priya Nair', email: 'priya@x.com', role: 'DISPATCHER', is_active: true, ...o };
}

describe('Team page', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(authApi.getOrgId).mockReturnValue('o1');
    vi.mocked(authApi.isAdmin).mockReturnValue(true);
  });

  it('shows an admins-only notice when the viewer is not an admin', () => {
    vi.mocked(authApi.isAdmin).mockReturnValue(false);
    render(<Team />);
    expect(screen.getByText(/admins only/i)).toBeInTheDocument();
    expect(usersApi.listOrgUsers).not.toHaveBeenCalled();
  });

  it('lists team members with their email and role', async () => {
    vi.mocked(usersApi.listOrgUsers).mockResolvedValue(ok([
      user(),
      user({ id: 'u2', name: 'Sam Roy', email: 'sam@x.com', role: 'WAREHOUSE_STAFF', is_active: false }),
    ]));
    render(<Team />);

    const priya = (await screen.findByText('Priya Nair')).closest('tr') as HTMLElement;
    expect(within(priya).getByLabelText(/role for priya nair/i)).toHaveValue('DISPATCHER');

    const sam = screen.getByText('Sam Roy').closest('tr') as HTMLElement;
    expect(within(sam).getByText('Inactive')).toBeInTheDocument();
  });

  it('adds a team member from the form and reloads', async () => {
    const u = userEvent.setup();
    vi.mocked(usersApi.listOrgUsers)
      .mockResolvedValueOnce(ok([]))
      .mockResolvedValueOnce(ok([user({ name: 'New Person' })]));
    vi.mocked(usersApi.createOrgUser).mockResolvedValue(ok(user({ name: 'New Person' })));

    render(<Team />);
    await screen.findByText(/no team members yet/i);

    await u.click(screen.getByRole('button', { name: /add member/i }));
    await u.type(screen.getByLabelText(/^name$/i), 'New Person');
    await u.type(screen.getByLabelText(/email/i), 'new@x.com');
    await u.type(screen.getByLabelText(/temporary password/i), 'password123');
    await u.selectOptions(screen.getByLabelText(/^role$/i), 'WAREHOUSE_STAFF');
    await u.click(screen.getByRole('button', { name: /^add member$/i }));

    expect(usersApi.createOrgUser).toHaveBeenCalledWith('o1', {
      name: 'New Person', email: 'new@x.com', password: 'password123', role: 'WAREHOUSE_STAFF',
    });
    await waitFor(() => expect(screen.getByText('New Person')).toBeInTheDocument());
  });

  it('changes a member role through the row dropdown', async () => {
    const u = userEvent.setup();
    vi.mocked(usersApi.listOrgUsers).mockResolvedValue(ok([user()]));
    vi.mocked(usersApi.updateOrgUser).mockResolvedValue(ok(user({ role: 'ADMIN' })));

    render(<Team />);
    const row = (await screen.findByText('Priya Nair')).closest('tr') as HTMLElement;
    await u.selectOptions(within(row).getByLabelText(/role for priya nair/i), 'ADMIN');

    expect(usersApi.updateOrgUser).toHaveBeenCalledWith('u1', {
      name: 'Priya Nair', role: 'ADMIN', is_active: true,
    });
  });

  it('shows an error message when adding a member fails', async () => {
    const u = userEvent.setup();
    vi.mocked(usersApi.listOrgUsers).mockResolvedValue(ok([]));
    vi.mocked(usersApi.createOrgUser).mockRejectedValue({
      response: { data: { message: 'That email address is already registered' } },
    });

    render(<Team />);
    await screen.findByText(/no team members yet/i);
    await u.click(screen.getByRole('button', { name: /add member/i }));
    await u.type(screen.getByLabelText(/^name$/i), 'Dup');
    await u.type(screen.getByLabelText(/email/i), 'dup@x.com');
    await u.type(screen.getByLabelText(/temporary password/i), 'password123');
    await u.click(screen.getByRole('button', { name: /^add member$/i }));

    expect(await screen.findByText(/already registered/i)).toBeInTheDocument();
  });
});
