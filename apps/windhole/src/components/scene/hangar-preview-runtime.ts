import * as THREE from 'three';
import type { BuiltInAircraftId } from './aircraft-blueprint';
import { createAircraft } from './aircraft-registry';
import type { PilotProfile } from './pilot-profile';
import type { WeaponLoadout } from './weapon-loadout';

export interface HangarPreviewConfiguration {
  airframeId: BuiltInAircraftId;
  candidate: string;
  pilotProfile: PilotProfile;
  loadout: WeaponLoadout;
}

export interface HangarPreviewRuntime {
  setConfiguration(configuration: HangarPreviewConfiguration): void;
  reset(): void;
  dispose(): void;
}

const AIRCRAFT_DESIGN_SIZE = 7.6;
const BASE_CAMERA_FOV = 34;
const DEFAULT_PITCH = THREE.MathUtils.degToRad(-7);
const DEFAULT_YAW = THREE.MathUtils.degToRad(-32);
const MAX_PITCH = THREE.MathUtils.degToRad(30);
const TURNTABLE_SPEED = THREE.MathUtils.degToRad(4.5);

interface DragState {
  pointerId: number;
  startX: number;
  startY: number;
  startPitch: number;
  startYaw: number;
}

export function createHangarPreviewRuntime(
  container: HTMLDivElement,
  initial: HangarPreviewConfiguration
): HangarPreviewRuntime {
  const renderer = new THREE.WebGLRenderer({
    alpha: true,
    antialias: true,
    powerPreference: 'high-performance',
  });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.setClearColor(0x070b11, 1);
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.08;
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;

  const canvas = renderer.domElement;
  canvas.setAttribute('aria-label', 'A3S智能体评测机库三维预览');
  canvas.setAttribute('role', 'application');
  canvas.style.cursor = 'grab';
  canvas.style.display = 'block';
  canvas.style.height = '100%';
  canvas.style.touchAction = 'none';
  canvas.style.width = '100%';
  container.appendChild(canvas);

  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0x070b11);
  scene.fog = new THREE.Fog(0x070b11, 16, 42);

  const camera = new THREE.PerspectiveCamera(BASE_CAMERA_FOV, 1, 0.1, 80);
  camera.position.set(0, 3.05, 13.8);
  camera.lookAt(0, 0.1, 0);

  const environment = createHangarEnvironment();
  scene.add(environment);

  const presentationPivot = new THREE.Group();
  presentationPivot.name = 'hangar-aircraft-presentation-pivot';
  presentationPivot.position.y = 0.15;
  scene.add(presentationPivot);

  let activeAircraft = createHangarPreviewAircraft(initial);
  presentationPivot.add(activeAircraft);
  let drag: DragState | undefined;
  let disposed = false;
  let animationFrame = 0;
  let previousFrameTime = performance.now();
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');

  const reset = () => {
    if (disposed) return;
    presentationPivot.rotation.set(DEFAULT_PITCH, DEFAULT_YAW, 0);
  };
  reset();

  const resize = () => {
    if (disposed) return;
    const width = Math.max(1, container.clientWidth);
    const height = Math.max(1, container.clientHeight);
    const aspect = width / height;
    camera.aspect = aspect;
    camera.fov = responsiveVerticalFov(aspect);
    camera.updateProjectionMatrix();
    renderer.setSize(width, height, false);
  };
  const resizeObserver = new ResizeObserver(resize);
  resizeObserver.observe(container);
  resize();

  const handlePointerDown = (event: PointerEvent) => {
    if (disposed || drag || (event.pointerType === 'mouse' && event.button !== 0)) return;
    event.preventDefault();
    canvas.setPointerCapture(event.pointerId);
    drag = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      startPitch: presentationPivot.rotation.x,
      startYaw: presentationPivot.rotation.y,
    };
    canvas.style.cursor = 'grabbing';
  };

  const handlePointerMove = (event: PointerEvent) => {
    if (drag?.pointerId !== event.pointerId) return;
    event.preventDefault();
    presentationPivot.rotation.y = drag.startYaw + (event.clientX - drag.startX) * 0.008;
    presentationPivot.rotation.x = THREE.MathUtils.clamp(
      drag.startPitch + (event.clientY - drag.startY) * 0.006,
      -MAX_PITCH,
      MAX_PITCH
    );
  };

  const endDrag = (event: PointerEvent) => {
    if (drag?.pointerId !== event.pointerId) return;
    const pointerId = drag.pointerId;
    drag = undefined;
    canvas.style.cursor = 'grab';
    if (event.type !== 'lostpointercapture' && canvas.hasPointerCapture(pointerId)) {
      canvas.releasePointerCapture(pointerId);
    }
  };

  const handleDoubleClick = (event: MouseEvent) => {
    event.preventDefault();
    reset();
  };

  canvas.addEventListener('pointerdown', handlePointerDown);
  canvas.addEventListener('pointermove', handlePointerMove);
  canvas.addEventListener('pointerup', endDrag);
  canvas.addEventListener('pointercancel', endDrag);
  canvas.addEventListener('lostpointercapture', endDrag);
  canvas.addEventListener('dblclick', handleDoubleClick);

  const animate = (time: number) => {
    if (disposed) return;
    const delta = Math.min(Math.max((time - previousFrameTime) / 1000, 0), 0.05);
    previousFrameTime = time;
    if (!drag && !reducedMotion.matches) presentationPivot.rotation.y += delta * TURNTABLE_SPEED;
    renderer.render(scene, camera);
    animationFrame = window.requestAnimationFrame(animate);
  };
  renderer.render(scene, camera);
  animationFrame = window.requestAnimationFrame(animate);

  return {
    setConfiguration: (configuration) => {
      if (disposed) return;
      const nextAircraft = createHangarPreviewAircraft(configuration);
      presentationPivot.remove(activeAircraft);
      disposeObjectResources(activeAircraft);
      activeAircraft = nextAircraft;
      presentationPivot.add(activeAircraft);
    },
    reset,
    dispose: () => {
      if (disposed) return;
      disposed = true;
      window.cancelAnimationFrame(animationFrame);
      resizeObserver.disconnect();

      canvas.removeEventListener('pointerdown', handlePointerDown);
      canvas.removeEventListener('pointermove', handlePointerMove);
      canvas.removeEventListener('pointerup', endDrag);
      canvas.removeEventListener('pointercancel', endDrag);
      canvas.removeEventListener('lostpointercapture', endDrag);
      canvas.removeEventListener('dblclick', handleDoubleClick);
      if (drag && canvas.hasPointerCapture(drag.pointerId)) canvas.releasePointerCapture(drag.pointerId);
      drag = undefined;

      presentationPivot.remove(activeAircraft);
      disposeObjectResources(activeAircraft);
      scene.remove(presentationPivot, environment);
      disposeObjectResources(environment);
      scene.clear();

      renderer.renderLists.dispose();
      renderer.dispose();
      renderer.forceContextLoss();
      canvas.remove();
    },
  };
}

export function createHangarPreviewAircraft(configuration: HangarPreviewConfiguration): THREE.Group {
  const aircraft = createAircraft(configuration.candidate, {
    airframeId: configuration.airframeId,
    candidateFamily: configuration.pilotProfile.candidateFamily,
    pilotProfile: configuration.pilotProfile,
    weaponLoadout: configuration.loadout,
  });

  try {
    aircraft.updateMatrixWorld(true);
    const bounds = new THREE.Box3().setFromObject(aircraft);
    if (bounds.isEmpty()) throw new Error('Aircraft preview has no visible geometry');

    const center = bounds.getCenter(new THREE.Vector3());
    const size = bounds.getSize(new THREE.Vector3());
    const maxDimension = Math.max(size.x, size.y, size.z);
    if (!Number.isFinite(maxDimension) || maxDimension <= 0) {
      throw new Error('Aircraft preview has invalid dimensions');
    }

    const scale = AIRCRAFT_DESIGN_SIZE / maxDimension;
    aircraft.scale.setScalar(scale);
    aircraft.position.copy(center).multiplyScalar(-scale);
    aircraft.traverse((object) => {
      if (object instanceof THREE.Mesh) {
        object.castShadow = true;
        object.receiveShadow = true;
      }
    });
    aircraft.updateMatrixWorld(true);
    return aircraft;
  } catch (error) {
    disposeObjectResources(aircraft);
    throw error;
  }
}

function createHangarEnvironment(): THREE.Group {
  const environment = new THREE.Group();
  environment.name = 'hangar-preview-environment';
  const floorY = -2.25;

  const floor = new THREE.Mesh(
    new THREE.PlaneGeometry(34, 30),
    new THREE.MeshStandardMaterial({ color: 0x111820, metalness: 0.7, roughness: 0.48 })
  );
  floor.name = 'hangar-floor';
  floor.position.y = floorY;
  floor.rotation.x = -Math.PI / 2;
  floor.receiveShadow = true;
  environment.add(floor);

  const backWall = new THREE.Mesh(
    new THREE.PlaneGeometry(34, 17),
    new THREE.MeshStandardMaterial({ color: 0x0d141d, metalness: 0.42, roughness: 0.72 })
  );
  backWall.name = 'hangar-back-wall';
  backWall.position.set(0, 5.6, -11.5);
  backWall.receiveShadow = true;
  environment.add(backWall);

  const grid = new THREE.GridHelper(30, 30, 0x537584, 0x223440);
  grid.name = 'hangar-floor-grid';
  grid.position.y = floorY + 0.012;
  for (const material of materialArray(grid.material)) {
    material.depthWrite = false;
    material.opacity = 0.34;
    material.transparent = true;
  }
  environment.add(grid);

  const deck = new THREE.Mesh(
    new THREE.BoxGeometry(10.6, 0.08, 7.2),
    new THREE.MeshStandardMaterial({
      color: 0x121c25,
      metalness: 0.82,
      roughness: 0.32,
    })
  );
  deck.name = 'aircraft-assembly-deck';
  deck.position.y = floorY + 0.045;
  deck.receiveShadow = true;
  environment.add(deck);

  addHangarArchitecture(environment, floorY);
  addHangarLighting(environment);
  return environment;
}

function addHangarArchitecture(environment: THREE.Group, floorY: number): void {
  const beamGeometry = new THREE.BoxGeometry(0.28, 12.5, 0.36);
  const beamMaterial = new THREE.MeshStandardMaterial({ color: 0x27343d, metalness: 0.76, roughness: 0.3 });
  for (const x of [-10.5, -6.8, 6.8, 10.5]) {
    const beam = new THREE.Mesh(beamGeometry, beamMaterial);
    beam.name = 'hangar-structural-column';
    beam.position.set(x, floorY + 6.25, -10.9);
    beam.castShadow = true;
    environment.add(beam);
  }

  const lightGeometry = new THREE.BoxGeometry(3.6, 0.045, 0.12);
  const lightMaterial = new THREE.MeshBasicMaterial({ color: 0x7ed8e7, toneMapped: false });
  for (const x of [-8.6, -4.3, 0, 4.3, 8.6]) {
    const lightBar = new THREE.Mesh(lightGeometry, lightMaterial);
    lightBar.name = 'hangar-ceiling-light';
    lightBar.position.set(x, 8.7, -7.8);
    environment.add(lightBar);
  }

  const guideGeometry = new THREE.BoxGeometry(0.055, 0.018, 7.5);
  const guideMaterial = new THREE.MeshBasicMaterial({
    color: 0x478ea1,
    opacity: 0.62,
    transparent: true,
  });
  for (const x of [-6.1, 6.1]) {
    const guide = new THREE.Mesh(guideGeometry, guideMaterial);
    guide.name = 'hangar-floor-guide-light';
    guide.position.set(x, floorY + 0.04, -2.4);
    environment.add(guide);
  }
}

function addHangarLighting(environment: THREE.Group): void {
  environment.add(new THREE.HemisphereLight(0x9bc9df, 0x18212a, 1.45));

  const key = new THREE.DirectionalLight(0xe9f5ff, 4.25);
  key.name = 'hangar-key-light';
  key.position.set(5.5, 10.5, 8);
  key.castShadow = true;
  key.shadow.bias = -0.00035;
  key.shadow.mapSize.set(1536, 1536);
  key.shadow.camera.left = -9;
  key.shadow.camera.right = 9;
  key.shadow.camera.top = 8;
  key.shadow.camera.bottom = -8;
  key.shadow.camera.near = 1;
  key.shadow.camera.far = 35;
  environment.add(key);

  const coolFill = new THREE.SpotLight(0x5fc8ff, 45, 34, Math.PI / 5, 0.55, 1.3);
  coolFill.name = 'hangar-cool-fill';
  coolFill.position.set(-9, 5.2, 5.5);
  coolFill.target.position.set(0, 0, 0);
  environment.add(coolFill, coolFill.target);

  const warmRim = new THREE.SpotLight(0xffa65c, 36, 30, Math.PI / 5.5, 0.62, 1.4);
  warmRim.name = 'hangar-warm-rim';
  warmRim.position.set(8.5, 4.2, -6.5);
  warmRim.target.position.set(0, 0.1, 0);
  environment.add(warmRim, warmRim.target);

  const padGlow = new THREE.PointLight(0x54bfd5, 8, 13, 1.8);
  padGlow.name = 'inspection-pad-glow';
  padGlow.position.set(0, -1.65, 0.5);
  environment.add(padGlow);
}

function responsiveVerticalFov(aspect: number): number {
  const minimumFramingAspect = 1.15;
  if (aspect >= minimumFramingAspect) return BASE_CAMERA_FOV;
  const baseRadians = THREE.MathUtils.degToRad(BASE_CAMERA_FOV);
  return THREE.MathUtils.radToDeg(
    2 * Math.atan((Math.tan(baseRadians / 2) * minimumFramingAspect) / Math.max(aspect, 0.1))
  );
}

function disposeObjectResources(root: THREE.Object3D): void {
  const geometries = new Set<THREE.BufferGeometry>();
  const materials = new Set<THREE.Material>();
  const textures = new Set<THREE.Texture>();

  root.traverse((object) => {
    if (object instanceof THREE.Mesh || object instanceof THREE.Line || object instanceof THREE.Points) {
      geometries.add(object.geometry);
      for (const material of materialArray(object.material)) materials.add(material);
    } else if (object instanceof THREE.Sprite) {
      materials.add(object.material);
    }
  });

  for (const material of materials) {
    for (const value of Object.values(material)) {
      if (value instanceof THREE.Texture) textures.add(value);
    }
    if (material instanceof THREE.ShaderMaterial) {
      for (const uniform of Object.values(material.uniforms)) collectTextures(uniform.value, textures);
    }
  }
  for (const texture of textures) texture.dispose();
  for (const material of materials) material.dispose();
  for (const geometry of geometries) geometry.dispose();
}

function collectTextures(value: unknown, textures: Set<THREE.Texture>): void {
  if (value instanceof THREE.Texture) {
    textures.add(value);
  } else if (Array.isArray(value)) {
    for (const item of value) collectTextures(item, textures);
  }
}

function materialArray(material: THREE.Material | THREE.Material[]): THREE.Material[] {
  return Array.isArray(material) ? material : [material];
}
