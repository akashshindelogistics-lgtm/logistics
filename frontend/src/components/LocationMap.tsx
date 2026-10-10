import { MapContainer, TileLayer, Marker, Popup } from 'react-leaflet';
import L from 'leaflet';
import 'leaflet/dist/leaflet.css';
import './LocationMap.css';

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
  /** Image to draw as the marker (e.g. a vehicle's 3D icon) instead of the default pin. */
  iconUrl?: string;
}

// One DivIcon per image, so re-renders don't rebuild every marker.
const imageIcons = new Map<string, L.DivIcon>();

/** A marker that is the image itself, standing on a small shadow, anchored at its base. */
export function imagePinIcon(url: string): L.DivIcon {
  let icon = imageIcons.get(url);
  if (!icon) {
    const img = document.createElement('img');
    img.src = url;
    img.alt = '';
    img.draggable = false;
    icon = L.divIcon({
      className: 'map-pin-3d',
      html: img,
      iconSize: [46, 46],
      iconAnchor: [23, 40],
      popupAnchor: [0, -34],
    });
    imageIcons.set(url, icon);
  }
  return icon;
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
        <Marker key={i} position={[pin.lat, pin.lng]} {...(pin.iconUrl ? { icon: imagePinIcon(pin.iconUrl) } : {})}>
          <Popup>
            <strong>{pin.label}</strong>
            {pin.detail && <><br />{pin.detail}</>}
          </Popup>
        </Marker>
      ))}
    </MapContainer>
  );
}
