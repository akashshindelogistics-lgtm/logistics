import { afterEach, describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Link, MemoryRouter, Route, Routes } from 'react-router-dom';
import { useRef } from 'react';
import { canViewTransition, useTransitionClick } from './viewTransition';

function Source() {
  const icon = useRef<HTMLImageElement>(null);
  const onClick = useTransitionClick('/next', { state: { from: 'list' }, morph: { name: 'hero', element: () => icon.current } });
  return (
    <>
      <img ref={icon} alt="icon" />
      <Link to="/next" onClick={onClick}>go</Link>
    </>
  );
}

const realMatchMedia = window.matchMedia;
function renderApp() {
  render(
    <MemoryRouter initialEntries={['/']}>
      <Routes>
        <Route path="/" element={<Source />} />
        <Route path="/next" element={<p>next page</p>} />
      </Routes>
    </MemoryRouter>,
  );
}

describe('view transitions', () => {
  afterEach(() => {
    window.matchMedia = realMatchMedia;
    delete (document as { startViewTransition?: unknown }).startViewTransition;
  });

  it('are off without the API or with reduced motion (as in tests)', () => {
    expect(canViewTransition()).toBe(false);
    (document as { startViewTransition?: unknown }).startViewTransition = vi.fn();
    expect(canViewTransition()).toBe(false); // reduced motion is on in the test setup
  });

  it('a plain navigation still happens when transitions are unavailable', async () => {
    renderApp();
    await userEvent.click(screen.getByRole('link', { name: 'go' }));
    expect(screen.getByText('next page')).toBeInTheDocument();
  });

  it('navigates inside startViewTransition and names the morphing element meanwhile', async () => {
    window.matchMedia = ((q: string) => ({ ...realMatchMedia(q), matches: false })) as typeof window.matchMedia;
    let resolveFinished!: () => void;
    let nameDuringTransition = '';
    (document as { startViewTransition?: unknown }).startViewTransition = vi.fn((update: () => void) => {
      nameDuringTransition = (screen.getByAltText('icon') as HTMLImageElement).style.viewTransitionName;
      update();
      return { finished: new Promise<void>(r => { resolveFinished = r; }) };
    });
    renderApp();

    await userEvent.click(screen.getByRole('link', { name: 'go' }));

    expect(document.startViewTransition).toHaveBeenCalledTimes(1);
    expect(nameDuringTransition).toBe('hero');
    expect(screen.getByText('next page')).toBeInTheDocument();
    resolveFinished();
  });
});
