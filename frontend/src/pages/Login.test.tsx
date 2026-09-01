import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import Login from './Login';
import * as authApi from '../api/auth';

const navigateMock = vi.fn();
vi.mock('react-router-dom', async importOriginal => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return { ...actual, useNavigate: () => navigateMock };
});
vi.mock('../api/auth');

const orgList = { data: { data: [{ id: 'o1', name: 'Express Freight' }, { id: 'o2', name: 'Blue Dart' }] } };

describe('Login page', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(authApi.isLoggedIn).mockReturnValue(false);
  });

  it('redirects to the dashboard if the visitor is already logged in', async () => {
    vi.mocked(authApi.isLoggedIn).mockReturnValue(true);
    render(<Login />);
    await waitFor(() => expect(navigateMock).toHaveBeenCalledWith('/', { replace: true }));
    expect(authApi.listAuthOrgs).not.toHaveBeenCalled();
  });

  it('lists the organizations returned by the API in the select', async () => {
    vi.mocked(authApi.listAuthOrgs).mockResolvedValue(orgList as never);
    render(<Login />);
    expect(await screen.findByRole('option', { name: 'Express Freight' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Blue Dart' })).toBeInTheDocument();
  });

  it('shows a "create one first" hint and disables sign-in when there are no orgs', async () => {
    vi.mocked(authApi.listAuthOrgs).mockResolvedValue({ data: { data: [] } } as never);
    render(<Login />);
    expect(await screen.findByText(/no organizations found/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sign in/i })).toBeDisabled();
  });

  it('stores the session and navigates to the org on a successful login', async () => {
    const user = userEvent.setup();
    vi.mocked(authApi.listAuthOrgs).mockResolvedValue(orgList as never);
    vi.mocked(authApi.login).mockResolvedValue({
      data: { success: true, data: { token: 't', org_id: 'o1', org_name: 'Express Freight' } },
    } as never);

    render(<Login />);
    await screen.findByRole('option', { name: 'Express Freight' });

    await user.selectOptions(screen.getByLabelText(/organization/i), 'o1');
    await user.type(screen.getByLabelText(/password/i), 'hunter2');
    await user.click(screen.getByRole('button', { name: /sign in/i }));

    expect(authApi.login).toHaveBeenCalledWith('o1', 'hunter2');
    await waitFor(() =>
      expect(authApi.storeAuth).toHaveBeenCalledWith({
        token: 't',
        org_id: 'o1',
        org_name: 'Express Freight',
      }),
    );
    expect(navigateMock).toHaveBeenCalledWith('/orgs/o1', { replace: true });
  });

  it('shows an invalid-credentials error when the login call rejects', async () => {
    const user = userEvent.setup();
    vi.mocked(authApi.listAuthOrgs).mockResolvedValue(orgList as never);
    vi.mocked(authApi.login).mockRejectedValue(new Error('401'));

    render(<Login />);
    await screen.findByRole('option', { name: 'Express Freight' });

    await user.selectOptions(screen.getByLabelText(/organization/i), 'o2');
    await user.type(screen.getByLabelText(/password/i), 'wrong');
    await user.click(screen.getByRole('button', { name: /sign in/i }));

    expect(await screen.findByText(/invalid credentials/i)).toBeInTheDocument();
    expect(authApi.storeAuth).not.toHaveBeenCalled();
    expect(navigateMock).not.toHaveBeenCalled();
  });
});
