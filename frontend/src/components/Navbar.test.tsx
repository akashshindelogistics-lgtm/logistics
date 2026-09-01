import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import Navbar from './Navbar';

describe('Navbar', () => {
  it('renders a link to every top-level section', () => {
    render(<Navbar />, { wrapper: MemoryRouter });

    expect(screen.getByText('Logistics System')).toBeInTheDocument();
    for (const [label, href] of [
      ['Dashboard', '/'],
      ['Organizations', '/orgs'],
      ['Vehicles', '/vehicles'],
      ['Customers', '/customers'],
      ['Dispatches', '/dispatches'],
    ]) {
      expect(screen.getByRole('link', { name: label })).toHaveAttribute('href', href);
    }
  });

  it('marks the active route with the "active" class', () => {
    render(
      <MemoryRouter initialEntries={['/vehicles']}>
        <Navbar />
      </MemoryRouter>,
    );
    expect(screen.getByRole('link', { name: 'Vehicles' })).toHaveClass('active');
    expect(screen.getByRole('link', { name: 'Dashboard' })).not.toHaveClass('active');
  });
});
