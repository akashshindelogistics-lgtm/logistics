import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import Icon3D, { ICONS_3D } from './Icon3D';

describe('Icon3D', () => {
  it('renders a decorative image of the requested size', () => {
    const { container } = render(<Icon3D name="truck" size={48} />);
    const img = container.querySelector('img')!;
    expect(img).toHaveAttribute('src', ICONS_3D.truck);
    expect(img).toHaveAttribute('alt', '');
    expect(img).toHaveAttribute('aria-hidden', 'true');
    expect(img).toHaveAttribute('width', '48');
    expect(img).toHaveAttribute('data-icon3d', 'truck');
  });

  it('adds the float class only when asked', () => {
    const { container, rerender } = render(<Icon3D name="package" />);
    expect(container.querySelector('img')).not.toHaveClass('icon-3d-float');
    rerender(<Icon3D name="package" float className="extra" />);
    expect(container.querySelector('img')).toHaveClass('icon-3d', 'icon-3d-float', 'extra');
  });
});
