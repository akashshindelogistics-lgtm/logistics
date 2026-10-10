import bell from '../assets/3d/bell.png';
import building from '../assets/3d/building.png';
import chart from '../assets/3d/chart.png';
import documentIcon from '../assets/3d/document.png';
import factory from '../assets/3d/factory.png';
import lorry from '../assets/3d/lorry.png';
import map from '../assets/3d/map.png';
import outbox from '../assets/3d/outbox.png';
import packageIcon from '../assets/3d/package.png';
import people from '../assets/3d/people.png';
import person from '../assets/3d/person.png';
import pin from '../assets/3d/pin.png';
import receipt from '../assets/3d/receipt.png';
import road from '../assets/3d/road.png';
import store from '../assets/3d/store.png';
import truck from '../assets/3d/truck.png';
import wrench from '../assets/3d/wrench.png';
import './Icon3D.css';

// Clay-style 3D icons from Microsoft Fluent Emoji (MIT), downscaled to 160px.
// Meant for sizes of ~32px and up (stat cards, empty states, heroes); the
// flat stroke icons in Icons.tsx stay in the sidebar and buttons.
export const ICONS_3D = {
  bell,
  building,
  chart,
  document: documentIcon,
  factory,
  lorry,
  map,
  outbox,
  package: packageIcon,
  people,
  person,
  pin,
  receipt,
  road,
  store,
  truck,
  wrench,
} as const;

export type Icon3DName = keyof typeof ICONS_3D;

interface Icon3DProps {
  name: Icon3DName;
  size?: number;
  className?: string;
  /** Gentle hover bob; off for icons that sit inside dense UI. */
  float?: boolean;
}

/** A decorative 3D icon. Always `alt=""`: the text beside it carries the meaning. */
export default function Icon3D({ name, size = 40, className, float }: Icon3DProps) {
  return (
    <img
      src={ICONS_3D[name]}
      width={size}
      height={size}
      alt=""
      aria-hidden="true"
      draggable={false}
      data-icon3d={name}
      className={['icon-3d', float ? 'icon-3d-float' : '', className ?? ''].filter(Boolean).join(' ')}
    />
  );
}
