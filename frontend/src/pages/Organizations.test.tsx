import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import Organizations from './Organizations';
import * as orgsApi from '../api/orgs';
import * as authApi from '../api/auth';
import type { Organization } from '../types';

const navigateMock = vi.fn();
vi.mock('react-router-dom', async importOriginal => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return { ...actual, useNavigate: () => navigateMock };
});
vi.mock('../api/orgs');
vi.mock('../api/auth');

const ok = <T,>(data: T) => ({ success: true, message: '', data });

function org(overrides: Partial<Organization> = {}): Organization {
  return {
    id: 'o1',
    name: 'Express Freight',
    address: '1 Dock Rd',
    vehicles: [],
    godowns: [],
    ...overrides,
  };
}

function renderPage() {
  return render(<Organizations />, { wrapper: MemoryRouter });
}

describe('Organizations page', () => {
  beforeEach(() => vi.resetAllMocks());
  afterEach(() => vi.unstubAllGlobals());

  it('redirects straight to the authenticated org detail without loading the list', async () => {
    vi.mocked(authApi.getOrgId).mockReturnValue('o1');
    renderPage();
    await waitFor(() => expect(navigateMock).toHaveBeenCalledWith('/orgs/o1', { replace: true }));
    expect(orgsApi.listOrgs).not.toHaveBeenCalled();
  });

  it('lists organizations when the visitor is not tied to one', async () => {
    vi.mocked(authApi.getOrgId).mockReturnValue(null);
    vi.mocked(orgsApi.listOrgs).mockResolvedValue(
      ok([org(), org({ id: 'o2', name: 'Blue Dart', vehicles: [{ registration_number: 'X', capacity: 1, unit: 'MetricTon' }] })]),
    );
    renderPage();

    expect(await screen.findByText('Express Freight')).toBeInTheDocument();
    expect(screen.getByText('Blue Dart')).toBeInTheDocument();
  });

  it('shows the empty state when there are no organizations', async () => {
    vi.mocked(authApi.getOrgId).mockReturnValue(null);
    vi.mocked(orgsApi.listOrgs).mockResolvedValue(ok([]));
    renderPage();
    expect(await screen.findByText(/no organizations found/i)).toBeInTheDocument();
  });

  it('deletes an organization after confirmation and drops it from the table', async () => {
    const user = userEvent.setup();
    vi.stubGlobal('confirm', vi.fn(() => true));
    vi.mocked(authApi.getOrgId).mockReturnValue(null);
    vi.mocked(orgsApi.listOrgs).mockResolvedValue(ok([org(), org({ id: 'o2', name: 'Blue Dart' })]));
    vi.mocked(orgsApi.deleteOrg).mockResolvedValue(ok(null));

    renderPage();
    await screen.findByText('Express Freight');
    await user.click(screen.getAllByRole('button', { name: /delete/i })[0]);

    expect(orgsApi.deleteOrg).toHaveBeenCalledWith('o1');
    await waitFor(() => expect(screen.queryByText('Express Freight')).not.toBeInTheDocument());
    expect(screen.getByText('Blue Dart')).toBeInTheDocument();
  });

  it('does not delete when the confirm prompt is dismissed', async () => {
    const user = userEvent.setup();
    vi.stubGlobal('confirm', vi.fn(() => false));
    vi.mocked(authApi.getOrgId).mockReturnValue(null);
    vi.mocked(orgsApi.listOrgs).mockResolvedValue(ok([org()]));

    renderPage();
    await screen.findByText('Express Freight');
    await user.click(screen.getByRole('button', { name: /delete/i }));

    expect(orgsApi.deleteOrg).not.toHaveBeenCalled();
  });
});
