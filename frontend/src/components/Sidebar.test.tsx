import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import Sidebar from './Sidebar';
import * as authApi from '../api/auth';

const navigateMock = vi.fn();
vi.mock('react-router-dom', async importOriginal => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return { ...actual, useNavigate: () => navigateMock };
});
vi.mock('../api/auth');

function renderSidebar() {
  return render(<Sidebar />, { wrapper: MemoryRouter });
}

describe('Sidebar', () => {
  beforeEach(() => vi.resetAllMocks());

  it('shows the org badge and a sign-out button when logged in', () => {
    vi.mocked(authApi.isLoggedIn).mockReturnValue(true);
    vi.mocked(authApi.getOrgName).mockReturnValue('Express Freight');
    vi.mocked(authApi.getOrgId).mockReturnValue('abcd1234efgh');
    renderSidebar();

    expect(screen.getByText('Express Freight')).toBeInTheDocument();
    expect(screen.getByText('abcd1234…')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sign out/i })).toBeInTheDocument();
  });

  it('shows the "API connected" footer and no badge when logged out', () => {
    vi.mocked(authApi.isLoggedIn).mockReturnValue(false);
    vi.mocked(authApi.getOrgName).mockReturnValue(null);
    vi.mocked(authApi.getOrgId).mockReturnValue(null);
    renderSidebar();

    expect(screen.getByText(/api connected/i)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /sign out/i })).not.toBeInTheDocument();
  });

  it('points the "My Organization" link at the specific org when the id is known', () => {
    vi.mocked(authApi.isLoggedIn).mockReturnValue(true);
    vi.mocked(authApi.getOrgName).mockReturnValue('Express Freight');
    vi.mocked(authApi.getOrgId).mockReturnValue('o1');
    renderSidebar();

    expect(screen.getByRole('link', { name: /my organization/i })).toHaveAttribute('href', '/orgs/o1');
  });

  it('falls back to /orgs for the organization link when no id is known', () => {
    vi.mocked(authApi.isLoggedIn).mockReturnValue(false);
    vi.mocked(authApi.getOrgName).mockReturnValue(null);
    vi.mocked(authApi.getOrgId).mockReturnValue(null);
    renderSidebar();

    expect(screen.getByRole('link', { name: /my organization/i })).toHaveAttribute('href', '/orgs');
  });

  it('clears the session and redirects to /login on sign out', async () => {
    const user = userEvent.setup();
    vi.mocked(authApi.isLoggedIn).mockReturnValue(true);
    vi.mocked(authApi.getOrgName).mockReturnValue('Express Freight');
    vi.mocked(authApi.getOrgId).mockReturnValue('o1');
    renderSidebar();

    await user.click(screen.getByRole('button', { name: /sign out/i }));

    expect(authApi.clearAuth).toHaveBeenCalled();
    expect(navigateMock).toHaveBeenCalledWith('/login', { replace: true });
  });
});
