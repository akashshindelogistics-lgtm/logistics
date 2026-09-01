import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import Register from './Register';
import * as orgsApi from '../api/orgs';
import * as authApi from '../api/auth';

const navigateMock = vi.fn();
vi.mock('react-router-dom', async importOriginal => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return { ...actual, useNavigate: () => navigateMock };
});
vi.mock('../api/orgs');
vi.mock('../api/auth');

function renderPage() {
  return render(<Register />, { wrapper: MemoryRouter });
}

async function fillForm(
  user: ReturnType<typeof userEvent.setup>,
  { password, confirm }: { password: string; confirm: string },
) {
  await user.type(screen.getByLabelText(/organization name/i), 'Express Freight');
  await user.type(screen.getByLabelText(/^address$/i), '1 Dock Rd');
  await user.type(screen.getByLabelText(/^password$/i), password);
  await user.type(screen.getByLabelText(/confirm password/i), confirm);
  await user.click(screen.getByRole('button', { name: /create organization/i }));
}

describe('Register page', () => {
  beforeEach(() => vi.resetAllMocks());

  it('rejects a form where the two passwords differ', async () => {
    const user = userEvent.setup();
    renderPage();
    await fillForm(user, { password: 'abcdef', confirm: 'abcdeXYZ' });

    expect(await screen.findByText(/passwords do not match/i)).toBeInTheDocument();
    expect(orgsApi.createOrg).not.toHaveBeenCalled();
  });

  it('rejects a password shorter than six characters', async () => {
    const user = userEvent.setup();
    renderPage();
    await fillForm(user, { password: 'abc', confirm: 'abc' });

    expect(await screen.findByText(/at least 6 characters/i)).toBeInTheDocument();
    expect(orgsApi.createOrg).not.toHaveBeenCalled();
  });

  it('creates the org, auto-logs in, and navigates to the new org detail', async () => {
    const user = userEvent.setup();
    vi.mocked(orgsApi.createOrg).mockResolvedValue({
      data: { success: true, data: { id: 'o9' } },
    } as never);
    vi.mocked(authApi.login).mockResolvedValue({
      data: { success: true, data: { token: 't', org_id: 'o9', org_name: 'Express Freight' } },
    } as never);

    renderPage();
    await fillForm(user, { password: 'abcdef', confirm: 'abcdef' });

    await waitFor(() =>
      expect(orgsApi.createOrg).toHaveBeenCalledWith('Express Freight', '1 Dock Rd', 'abcdef'),
    );
    expect(authApi.login).toHaveBeenCalledWith('o9', 'abcdef');
    await waitFor(() =>
      expect(navigateMock).toHaveBeenCalledWith('/orgs/o9', { replace: true }),
    );
    expect(authApi.storeAuth).toHaveBeenCalled();
  });

  it('surfaces an error when creating the organization fails', async () => {
    const user = userEvent.setup();
    vi.mocked(orgsApi.createOrg).mockRejectedValue(new Error('boom'));

    renderPage();
    await fillForm(user, { password: 'abcdef', confirm: 'abcdef' });

    expect(await screen.findByText(/failed to create organization/i)).toBeInTheDocument();
    expect(navigateMock).not.toHaveBeenCalled();
  });
});
