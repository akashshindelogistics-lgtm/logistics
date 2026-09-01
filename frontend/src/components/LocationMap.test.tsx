import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import LocationMap, { type MapPin } from './LocationMap';

// react-leaflet needs a real DOM/canvas; stub it down to something inspectable.
vi.mock('react-leaflet', () => ({
  MapContainer: ({ center, zoom, children }: { center: [number, number]; zoom: number; children: React.ReactNode }) => (
    <div data-testid="map" data-center={center.join(',')} data-zoom={zoom}>
      {children}
    </div>
  ),
  TileLayer: () => <div data-testid="tile-layer" />,
  Marker: ({ position, children }: { position: [number, number]; children: React.ReactNode }) => (
    <div data-testid="marker" data-position={position.join(',')}>
      {children}
    </div>
  ),
  Popup: ({ children }: { children: React.ReactNode }) => <div data-testid="popup">{children}</div>,
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
});
