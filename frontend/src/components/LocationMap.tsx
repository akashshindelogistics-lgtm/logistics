import { useEffect, useRef, useState } from 'react';
import { MapContainer, TileLayer, Marker, Popup, Polyline } from 'react-leaflet';
import L from 'leaflet';
import { prefersReducedMotion } from '../lib/motion';
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
  /**
   * Stable identity (e.g. a registration number). With it, a pin whose
   * position changes between renders glides to the new spot instead of the
   * marker being rebuilt in place.
   */
  id?: string;
}

/** How long a moved marker takes to glide to its new position. */
export const GLIDE_MS = 1200;
/** Jumps longer than this (a stale fix, a teleport) snap instead of gliding. */
export const GLIDE_MAX_METERS = 50_000;

/**
 * A Marker that eases from its previous position to a new one. react-leaflet
 * would `setLatLng` straight to the new position; this keeps the first
 * position as the prop and moves the Leaflet marker itself, frame by frame.
 */
function GlidingMarker({ pin }: { pin: MapPin }) {
  const ref = useRef<L.Marker>(null);
  const [initial] = useState<[number, number]>(() => [pin.lat, pin.lng]);

  useEffect(() => {
    const marker = ref.current;
    if (!marker) return;
    const from = marker.getLatLng();
    const to = L.latLng(pin.lat, pin.lng);
    if (from.equals(to)) return;
    if (prefersReducedMotion() || document.hidden || from.distanceTo(to) > GLIDE_MAX_METERS) {
      marker.setLatLng(to);
      return;
    }
    const start = performance.now();
    let frame = 0;
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / GLIDE_MS);
      const eased = 1 - Math.pow(1 - t, 3);
      marker.setLatLng([from.lat + (to.lat - from.lat) * eased, from.lng + (to.lng - from.lng) * eased]);
      if (t < 1) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    // Interrupted by a newer fix: stop here; the next glide starts from wherever the marker is.
    return () => cancelAnimationFrame(frame);
  }, [pin.lat, pin.lng]);

  return (
    <Marker ref={ref} position={initial} {...(pin.iconUrl ? { icon: imagePinIcon(pin.iconUrl) } : {})}>
      <Popup>
        <strong>{pin.label}</strong>
        {pin.detail && <><br />{pin.detail}</>}
      </Popup>
    </Marker>
  );
}

/**
 * A route line that draws itself from start to end when it appears. The path
 * gets pathLength=1 so the CSS dash animation is independent of its pixel length.
 */
function DrawnRoute({ points }: { points: [number, number][] }) {
  const ref = useRef<L.Polyline>(null);
  useEffect(() => {
    ref.current?.getElement()?.setAttribute('pathLength', '1');
  }, [points]);
  return <Polyline ref={ref} positions={points} pathOptions={{ className: 'map-route', color: '#3b82f6', weight: 3, opacity: 0.85 }} />;
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
  /** Points to join with a self-drawing line, in travel order (e.g. a trip's stops). */
  route?: [number, number][];
}

export default function LocationMap({ pins, height = '400px', route }: LocationMapProps) {
  const center: [number, number] =
    pins.length > 0 ? [pins[0].lat, pins[0].lng] : [20.5937, 78.9629];

  return (
    <MapContainer center={center} zoom={pins.length > 0 ? 10 : 5} style={{ height, width: '100%', borderRadius: '8px' }}>
      <TileLayer
        url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png"
        attribution='&copy; <a href="https://openstreetmap.org">OpenStreetMap</a>'
      />
      {route && route.length > 1 && <DrawnRoute points={route} />}
      {pins.map((pin, i) => <GlidingMarker key={pin.id ?? `${i}:${pin.label}`} pin={pin} />)}
    </MapContainer>
  );
}
