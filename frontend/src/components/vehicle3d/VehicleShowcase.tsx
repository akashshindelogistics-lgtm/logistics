import { Component, Suspense, lazy, useState, type CSSProperties, type ReactNode } from 'react';
import type { VehicleType } from '../../types';
import { ACTIVITY_STYLE, vehicleVisual, type VehicleActivity } from '../../lib/vehicleVisuals';
import { can3D } from '../../lib/webgl';
import './VehicleShowcase.css';

// three.js and the scene load only when this component decides to show 3D.
const VehicleScene = lazy(() => import('./VehicleScene'));

/** Falls back to the poster if the model or WebGL fails at runtime. */
class SceneBoundary extends Component<{ children: ReactNode; onError: () => void }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch() { this.props.onError(); }
  render() { return this.state.failed ? null : this.props.children; }
}

interface Props {
  type?: VehicleType | null;
  activity: VehicleActivity;
}

/**
 * The vehicle's model on a turntable with a status ring, or — without WebGL,
 * while it loads, or with 3D switched off — a still render of the same model.
 */
export default function VehicleShowcase({ type, activity }: Props) {
  const visual = vehicleVisual(type);
  const style = ACTIVITY_STYLE[activity];
  const [use3D, setUse3D] = useState(can3D);
  const [ready, setReady] = useState(false);

  return (
    <div
      className={`vehicle-showcase${ready ? ' is-ready' : ''}`}
      style={{ '--status': style.color } as CSSProperties}
      data-testid="vehicle-showcase"
      data-mode={use3D ? '3d' : 'poster'}
      data-activity={activity}
    >
      <div className="vehicle-showcase-stage">
        <img className="vehicle-showcase-poster" src={visual.icon} alt={`${visual.label} illustration`} />
        {use3D && (
          <SceneBoundary onError={() => setUse3D(false)}>
            <Suspense fallback={null}>
              <div className="vehicle-showcase-canvas">
                <VehicleScene
                  type={visual.type}
                  ringColor={style.color}
                  moving={style.moving}
                  onReady={() => setReady(true)}
                />
              </div>
            </Suspense>
          </SceneBoundary>
        )}
      </div>
      <div className="vehicle-showcase-meta">
        <span className="vehicle-showcase-status" data-testid="vehicle-activity">
          <span className="vehicle-showcase-dot" />{style.label}
        </span>
        <span className="vehicle-showcase-type">
          <strong>{visual.label}</strong> · {visual.description}
        </span>
        {use3D && <span className="vehicle-showcase-hint">Drag to rotate</span>}
      </div>
    </div>
  );
}
