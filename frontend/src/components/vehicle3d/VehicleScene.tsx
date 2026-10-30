import { Suspense, useEffect, useMemo, useRef } from 'react';
import { Canvas, useFrame, useLoader, useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import type { VehicleType } from '../../types';
import { vehicleVisual } from '../../lib/vehicleVisuals';

// The interactive 3D view of one vehicle. Kept in its own module so the
// page imports it with React.lazy: three.js only downloads when a vehicle
// detail page actually shows a model.

export interface VehicleSceneProps {
  type: VehicleType;
  /** Status color for the ring under the vehicle; omit for no ring. */
  ringColor?: string;
  /** Spin the wheels, for a vehicle that is in transit. */
  moving?: boolean;
  /** Turntable rotation plus drag-to-orbit. Off for still renders. */
  interactive?: boolean;
  /** Called once the model has loaded and drawn its first frames. */
  onReady?: () => void;
}

const prefersReducedMotion = () =>
  typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

/** Every model is scaled so its longest side is this many world units. */
const MODEL_LENGTH = 2.6;

/**
 * Build a tanker body for the flatbed model: a horizontal cylinder with
 * domed ends, sized to sit on the rear deck. `box` is the vehicle's bounds
 * after normalisation (front of the cab faces +Z).
 */
function makeTank(box: THREE.Box3): THREE.Group {
  const size = box.getSize(new THREE.Vector3());
  const radius = size.x * 0.36;
  const length = size.z * 0.56;
  const material = new THREE.MeshStandardMaterial({ color: '#d7dde4', metalness: 0.55, roughness: 0.3 });
  const band = new THREE.MeshStandardMaterial({ color: '#f59e0b', metalness: 0.2, roughness: 0.5 });

  const tank = new THREE.Group();
  const body = new THREE.Mesh(new THREE.CylinderGeometry(radius, radius, length, 32), material);
  body.rotation.x = Math.PI / 2;
  tank.add(body);
  for (const end of [-1, 1]) {
    const cap = new THREE.Mesh(new THREE.SphereGeometry(radius, 32, 16, 0, Math.PI * 2, 0, Math.PI / 2), material);
    cap.scale.y = 0.35;
    cap.rotation.x = (end * Math.PI) / 2;
    cap.position.z = (end * length) / 2;
    tank.add(cap);
  }
  for (const z of [-length / 4, length / 4]) {
    const ring = new THREE.Mesh(new THREE.TorusGeometry(radius * 1.01, radius * 0.06, 8, 32), band);
    ring.position.z = z;
    tank.add(ring);
  }
  const hatch = new THREE.Mesh(new THREE.CylinderGeometry(radius * 0.28, radius * 0.28, radius * 0.25, 20), material);
  hatch.position.y = radius;
  tank.add(hatch);
  tank.traverse(o => { o.castShadow = true; });

  // Rear deck: behind the cab, resting on the bed (~45% up the body).
  tank.position.set(0, box.min.y + size.y * 0.42 + radius, box.min.z + size.z * 0.34);
  return tank;
}

function Model({ type, moving, onReady }: Pick<VehicleSceneProps, 'type' | 'moving' | 'onReady'>) {
  const visual = vehicleVisual(type);
  const { scene } = useLoader(GLTFLoader, visual.modelUrl);

  const { root, wheels } = useMemo(() => {
    const model = scene.clone(true);
    for (const name of visual.hide ?? []) {
      model.getObjectByName(name)?.removeFromParent();
    }
    model.traverse(o => {
      if ((o as THREE.Mesh).isMesh) {
        o.castShadow = true;
        o.receiveShadow = true;
      }
    });

    // Normalise: longest side MODEL_LENGTH, centred on the origin, wheels on y=0.
    const box = new THREE.Box3().setFromObject(model);
    const size = box.getSize(new THREE.Vector3());
    const scale = MODEL_LENGTH / Math.max(size.x, size.y, size.z);
    model.scale.setScalar(scale);
    box.setFromObject(model);
    const center = box.getCenter(new THREE.Vector3());
    model.position.set(-center.x, -box.min.y, -center.z);

    const group = new THREE.Group();
    group.add(model);
    if (visual.tank) {
      group.add(makeTank(new THREE.Box3().setFromObject(model)));
    }
    const wheelNodes: THREE.Object3D[] = [];
    model.traverse(o => { if (o.name.startsWith('wheel')) wheelNodes.push(o); });
    return { root: group, wheels: wheelNodes };
  }, [scene, visual]);

  useFrame((_, delta) => {
    if (moving) for (const w of wheels) w.rotation.x += delta * 6;
  });

  // Report ready after a couple of frames so the first paint has happened.
  const frames = useRef(0);
  const reported = useRef(false);
  useFrame(() => {
    frames.current += 1;
    if (!reported.current && frames.current > 2) {
      reported.current = true;
      onReady?.();
    }
  });
  useEffect(() => { frames.current = 0; reported.current = false; }, [type]);

  return <primitive object={root} />;
}

function StatusRing({ color }: { color: string }) {
  return (
    <group rotation-x={-Math.PI / 2} position-y={0.004}>
      <mesh>
        <circleGeometry args={[1.75, 64]} />
        <meshBasicMaterial color={color} transparent opacity={0.12} depthWrite={false} />
      </mesh>
      <mesh>
        <ringGeometry args={[1.66, 1.75, 96]} />
        <meshBasicMaterial color={color} transparent opacity={0.9} depthWrite={false} />
      </mesh>
    </group>
  );
}

/** Slow auto-rotation plus drag-to-orbit, using three's own OrbitControls. */
function Turntable() {
  const { camera, gl } = useThree();
  const controls = useMemo(() => {
    const c = new OrbitControls(camera, gl.domElement);
    c.target.set(0, 0.45, 0);
    c.enablePan = false;
    c.enableZoom = false;
    c.enableDamping = true;
    c.autoRotate = !prefersReducedMotion();
    c.autoRotateSpeed = 1.4;
    c.minPolarAngle = Math.PI / 5;
    c.maxPolarAngle = Math.PI / 2.2;
    return c;
  }, [camera, gl]);
  useEffect(() => () => controls.dispose(), [controls]);
  useFrame(() => controls.update());
  return null;
}

export default function VehicleScene({ type, ringColor, moving, interactive = true, onReady }: VehicleSceneProps) {
  return (
    <Canvas
      shadows="percentage"
      dpr={[1, 2]}
      camera={{ position: [3.4, 2.1, 3.4], fov: 34 }}
      gl={{ alpha: true, antialias: true, preserveDrawingBuffer: !interactive }}
      onCreated={({ camera }) => camera.lookAt(0, 0.45, 0)}
      data-testid="vehicle-3d-canvas"
    >
      <ambientLight intensity={0.9} />
      <hemisphereLight args={['#ffffff', '#b0bccc', 0.7]} />
      <directionalLight
        position={[4, 7, 3]}
        intensity={1.8}
        castShadow
        shadow-mapSize={[1024, 1024]}
      />
      <Suspense fallback={null}>
        <Model type={type} moving={moving} onReady={onReady} />
      </Suspense>
      {ringColor && <StatusRing color={ringColor} />}
      {/* Invisible ground that only shows the vehicle's shadow */}
      <mesh rotation-x={-Math.PI / 2} receiveShadow>
        <planeGeometry args={[8, 8]} />
        <shadowMaterial transparent opacity={0.22} />
      </mesh>
      {interactive && <Turntable />}
    </Canvas>
  );
}
