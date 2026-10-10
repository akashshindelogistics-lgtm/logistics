import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import LocationMap, { imagePinIcon, type MapPin } from './LocationMap';

// react-leaflet needs a real DOM/canvas; stub it down to something inspectable.
vi.mock('react-leaflet', () => ({
  MapContainer: ({ center, zoom, children }: { center: [number, number]; zoom: number; children: React.ReactNode }) => (
    <div data-testid="map" data-center={center.join(',')} data-zoom={zoom}>
      {children}
    </div>
  ),
  TileLayer: () => <div data-testid="tile-layer" />,
  Marker: ({ position, icon, children }: { position: [number, number]; icon?: { options: { className?: string } }; children: React.ReactNode }) => (
    <div data-testid="marker" data-position={position.join(',')} data-icon-class={icon?.options.className ?? 'default'}>
      {children}
    </div>
  ),
  Popup: ({ children }: { children: React.ReactNode }) => <div data-testid="popup">{children}</div>,
  Polyline: ({ positions, pathOptions }: { positions: [number, number][]; pathOptions?: { className?: string } }) => (
    <div data-testid="route" data-points={positions.length} data-class={pathOptions?.className} />
  ),
}));

const pin = (overrides: Partial<MapPin> = {}): MapPin => ({
  lat: 19.076,
  lng: 72.877,
  label: 'Mumbai Depot',
  ...overrides,
});

describe('LocationMap', () => {
  it('centers on India at a low zoom when there are no pins', () => {
    render(<LocationMap pins={[]} />);
    const map = screen.getByTestId('map');
    expect(map).toHaveAttribute('data-center', '20.5937,78.9629');
    expect(map).toHaveAttribute('data-zoom', '5');
  });

  it('centers on the first pin at a closer zoom when pins are present', () => {
    render(<LocationMap pins={[pin(), pin({ lat: 28.6, lng: 77.2, label: 'Delhi' })]} />);
    const map = screen.getByTestId('map');
    expect(map).toHaveAttribute('data-center', '19.076,72.877');
    expect(map).toHaveAttribute('data-zoom', '10');
  });

  it('renders one marker per pin with its label and optional detail', () => {
    render(<LocationMap pins={[pin({ detail: '12 MT' }), pin({ label: 'Pune', detail: undefined })]} />);
    const markers = screen.getAllByTestId('marker');
    expect(markers).toHaveLength(2);
    expect(screen.getByText('Mumbai Depot')).toBeInTheDocument();
    expect(screen.getByText('12 MT')).toBeInTheDocument();
    expect(screen.getByText('Pune')).toBeInTheDocument();
  });

  it('draws a pin with iconUrl as an image marker and keeps the default pin otherwise', () => {
    render(<LocationMap pins={[pin({ iconUrl: '/truck.png' }), pin({ label: 'Plain' })]} />);
    const [withIcon, plain] = screen.getAllByTestId('marker');
    expect(withIcon).toHaveAttribute('data-icon-class', 'map-pin-3d');
    expect(plain).toHaveAttribute('data-icon-class', 'default');
  });

  it('imagePinIcon wraps the image and reuses one icon per URL', () => {
    const icon = imagePinIcon('/tanker.png');
    expect((icon.options.html as HTMLImageElement).getAttribute('src')).toBe('/tanker.png');
    expect(icon.options.iconAnchor).toEqual([23, 40]);
    expect(imagePinIcon('/tanker.png')).toBe(icon);
  });

  it('draws a self-drawing route only when it has at least two points', () => {
    const { rerender } = render(<LocationMap pins={[pin()]} route={[[18.5, 73.8]]} />);
    expect(screen.queryByTestId('route')).not.toBeInTheDocument();
    rerender(<LocationMap pins={[pin()]} route={[[18.5, 73.8], [18.6, 73.9], [18.7, 73.7]]} />);
    expect(screen.getByTestId('route')).toHaveAttribute('data-points', '3');
    expect(screen.getByTestId('route')).toHaveAttribute('data-class', 'map-route');
  });

  it('keeps a pin with an id as the same marker when its position changes', () => {
    const { rerender } = render(<LocationMap pins={[pin({ id: 'MH12', label: 'Truck' })]} />);
    const before = screen.getByTestId('marker');
    rerender(<LocationMap pins={[pin({ id: 'MH12', label: 'Truck', lat: 19.1, lng: 72.9 })]} />);
    // Same element: the marker is moved (glided), not torn down and rebuilt.
    expect(screen.getByTestId('marker')).toBe(before);
  });
});
