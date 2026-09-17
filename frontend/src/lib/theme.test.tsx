import { describe, it, expect, beforeEach, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ThemeProvider, useTheme } from './theme';

function Probe() {
  const { theme, toggleTheme } = useTheme();
  return (
    <button onClick={toggleTheme}>{theme}</button>
  );
}

function mockMatchMedia(matches: boolean) {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    configurable: true,
    value: vi.fn().mockReturnValue({ matches } as MediaQueryList),
  });
}

describe('ThemeProvider / useTheme', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute('data-theme');
  });

  it('defaults to light when there is no stored preference and no system preference', () => {
    mockMatchMedia(false);
    render(<ThemeProvider><Probe /></ThemeProvider>);

    expect(screen.getByRole('button')).toHaveTextContent('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
  });

  it('defaults to dark when the system prefers dark and nothing is stored', () => {
    mockMatchMedia(true);
    render(<ThemeProvider><Probe /></ThemeProvider>);

    expect(screen.getByRole('button')).toHaveTextContent('dark');
  });

  it('honors a previously stored theme over the system preference', () => {
    localStorage.setItem('logi_theme', 'dark');
    mockMatchMedia(false);
    render(<ThemeProvider><Probe /></ThemeProvider>);

    expect(screen.getByRole('button')).toHaveTextContent('dark');
  });

  it('toggles the theme, updates the DOM attribute, and persists to localStorage', async () => {
    const user = userEvent.setup();
    mockMatchMedia(false);
    render(<ThemeProvider><Probe /></ThemeProvider>);

    await user.click(screen.getByRole('button'));

    expect(screen.getByRole('button')).toHaveTextContent('dark');
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
    expect(localStorage.getItem('logi_theme')).toBe('dark');

    await user.click(screen.getByRole('button'));
    expect(screen.getByRole('button')).toHaveTextContent('light');
    expect(localStorage.getItem('logi_theme')).toBe('light');
  });

  it('throws when useTheme is used outside a ThemeProvider', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => render(<Probe />)).toThrow('useTheme must be used within a ThemeProvider');
    spy.mockRestore();
  });
});
