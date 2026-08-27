import { MapContainer, TileLayer, Marker, Popup } from 'react-leaflet';
import L from 'leaflet';
import 'leaflet/dist/leaflet.css';

// Fix default marker icons broken by webpack/vite asset hashing
delete (L.Icon.Default.prototype as unknown as Record<string, unknown>)._getIconUrl;
L.Icon.Default.mergeOptions({
  iconRetinaUrl: 'https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon-2x.png',
  iconUrl: 'https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon.png',
  shadowUrl: 'https://unpkg.com/leaflet@1.9.4/dist/images/marker-shadow.png',
});

export interface MapPin {
  lat: number;
  lng: number;
  label: string;
  detail?: string;
}

interface LocationMapProps {
  pins: MapPin[];
  height?: string;
}

export default function LocationMap({ pins, height = '400px' }: LocationMapProps) {
  const center: [number, number] =
    pins.length > 0 ? [pins[0].lat, pins[0].lng] : [20.5937, 78.9629];

  return (
    <MapContainer center={center} zoom={pins.length > 0 ? 10 : 5} style={{ height, width: '100%', borderRadius: '8px' }}>
      <TileLayer
        url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
        attribution='&copy; <a href="https://openstreetmap.org">OpenStreetMap</a>'
      />
      {pins.map((pin, i) => (
        <Marker key={i} position={[pin.lat, pin.lng]}>
          <Popup>
            <strong>{pin.label}</strong>
            {pin.detail && <><br />{pin.detail}</>}
          </Popup>
        </Marker>
      ))}
    </MapContainer>
  );
}
