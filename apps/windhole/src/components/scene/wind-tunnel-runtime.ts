import * as THREE from 'three';
import type { WindTunnelParameters } from '../../types/bench';
import {
  type AircraftFleet,
  applyAircraftTaskPose,
  createAircraftFleet,
  disposeAircraftFleet,
  updateAircraftFleet,
} from './aircraft-fleet';
import {
  type AircraftHoverEvent,
  type AircraftInteractionController,
  attachAircraftInteraction,
} from './aircraft-interaction';
import { createAircraftSpotlight, updateAircraftSpotlight } from './aircraft-spotlight';
import {
  createBattlefieldEnvironment,
  disposeBattlefieldEnvironment,
  setBattlefieldTheater,
  updateBattlefieldEnvironment,
} from './battlefield-environment';
import type { FormationAircraft } from './flight-formation';
import { taskWeather } from './task-weather';
import { createWeatherSystem, setWeatherPreset, updateWeatherSystem } from './weather-system';
import { createWindField, updateWindField } from './wind-field';

interface WindTunnelRuntimeOptions {
  formation: readonly FormationAircraft[];
  selectedId: string;
  taskId: string;
  taskCategory?: string;
  getParameters: () => WindTunnelParameters;
  onHover: (event?: AircraftHoverEvent) => void;
  onSelect: (id: string) => void;
}

export interface WindTunnelRuntime {
  resetAircraft: (id?: string) => void;
  setTask: (taskId: string, taskCategory?: string) => void;
  syncFormation: (formation: readonly FormationAircraft[], selectedId: string) => void;
  dispose: () => void;
}

export function createWindTunnelRuntime(
  container: HTMLDivElement,
  {
    formation,
    selectedId: initialSelection,
    taskId,
    taskCategory,
    getParameters,
    onHover,
    onSelect,
  }: WindTunnelRuntimeOptions
): WindTunnelRuntime {
  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, powerPreference: 'high-performance' });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.setClearColor(0x071014, 0);
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.08;
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  renderer.domElement.setAttribute('role', 'application');
  container.appendChild(renderer.domElement);

  const scene = new THREE.Scene();
  scene.fog = new THREE.FogExp2(0x071014, 0.021);
  const camera = new THREE.PerspectiveCamera(38, 1, 0.1, 120);
  camera.position.set(-8.8, 5.5, 15.2);
  camera.lookAt(0, -0.05, 0);

  const battlefield = createBattlefieldEnvironment(scene, taskId, taskCategory);
  const windField = createWindField(scene);
  const weatherSystem = createWeatherSystem(scene);
  setWeatherPreset(weatherSystem, taskWeather(taskId), true);

  let fleet = createAircraftFleet(formation);
  scene.add(fleet.group);
  let activeTaskId = taskId;
  let activeTaskCategory = taskCategory;
  applyAircraftTaskPose(fleet, activeTaskId, true);
  let selectedId = fleet.instances.some((instance) => instance.descriptor.instanceId === initialSelection)
    ? initialSelection
    : (fleet.instances[0]?.descriptor.instanceId ?? '');
  const spotlight = createAircraftSpotlight();
  scene.add(spotlight.light, spotlight.target);
  updateAircraftSpotlight(spotlight, fleet, selectedId, true);
  let signature = formationSignature(formation);
  let interaction = createInteraction(renderer.domElement, camera, fleet, selectedId, handleSelect, onHover);
  let disposed = false;

  const clock = new THREE.Clock();
  const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  let elapsed = 0;
  let animationFrame = 0;

  const resize = () => {
    const width = Math.max(1, container.clientWidth);
    const height = Math.max(1, container.clientHeight);
    camera.aspect = width / height;
    camera.fov = camera.aspect < 1.25 ? 44 : 38;
    camera.updateProjectionMatrix();
    renderer.setSize(width, height, false);
  };
  const resizeObserver = new ResizeObserver(resize);
  resizeObserver.observe(container);
  resize();

  const animate = () => {
    if (disposed) return;
    animationFrame = window.requestAnimationFrame(animate);
    const delta = Math.min(clock.getDelta(), 0.05);
    const parameters = getParameters();
    const motionScale = reducedMotion ? 0.18 : 1;
    const simulationDelta = parameters.paused ? 0 : delta * motionScale;
    elapsed += simulationDelta;

    updateAircraftFleet(fleet, parameters, elapsed, motionScale);
    updateAircraftSpotlight(spotlight, fleet, selectedId);
    updateBattlefieldEnvironment(battlefield, elapsed, simulationDelta);
    updateWindField(windField, parameters, fleet.centers, elapsed, simulationDelta);
    updateWeatherSystem(weatherSystem, elapsed, delta * motionScale);
    renderer.render(scene, camera);
  };
  animate();

  function handleSelect(id: string): void {
    selectedId = id;
    onSelect(id);
  }

  function selectAircraft(id: string): void {
    if (!fleet.instances.some((instance) => instance.descriptor.instanceId === id)) return;
    selectedId = id;
    interaction.syncSelection(id);
  }

  function setTask(nextTaskId: string, nextTaskCategory?: string): void {
    if (nextTaskId === activeTaskId && nextTaskCategory === activeTaskCategory) return;
    activeTaskId = nextTaskId;
    activeTaskCategory = nextTaskCategory;
    applyAircraftTaskPose(fleet, activeTaskId);
    setBattlefieldTheater(battlefield, activeTaskId, activeTaskCategory);
    setWeatherPreset(weatherSystem, taskWeather(activeTaskId));
  }

  function syncFormation(nextFormation: readonly FormationAircraft[], requestedSelection: string): void {
    const nextSignature = formationSignature(nextFormation);
    if (nextSignature === signature) {
      if (requestedSelection && requestedSelection !== selectedId) selectAircraft(requestedSelection);
      return;
    }

    interaction.dispose();
    scene.remove(fleet.group);
    disposeAircraftFleet(fleet);
    fleet = createAircraftFleet(nextFormation);
    scene.add(fleet.group);
    applyAircraftTaskPose(fleet, activeTaskId, true);
    signature = nextSignature;
    selectedId = fleet.instances.some((instance) => instance.descriptor.instanceId === requestedSelection)
      ? requestedSelection
      : (fleet.instances[0]?.descriptor.instanceId ?? '');
    updateAircraftSpotlight(spotlight, fleet, selectedId, true);
    interaction = createInteraction(renderer.domElement, camera, fleet, selectedId, handleSelect, onHover);
  }

  return {
    resetAircraft: (id) => interaction.reset(id),
    setTask,
    syncFormation,
    dispose: () => {
      disposed = true;
      window.cancelAnimationFrame(animationFrame);
      resizeObserver.disconnect();
      interaction.dispose();
      scene.remove(fleet.group);
      disposeAircraftFleet(fleet);
      disposeBattlefieldEnvironment(battlefield);
      scene.traverse((object) => {
        if (object instanceof THREE.Mesh || object instanceof THREE.Line || object instanceof THREE.Points) {
          object.geometry.dispose();
          disposeMaterial(object.material);
        }
      });
      renderer.dispose();
      renderer.domElement.remove();
    },
  };
}

function createInteraction(
  canvas: HTMLCanvasElement,
  camera: THREE.Camera,
  fleet: AircraftFleet,
  selectedId: string,
  onSelect: (id: string) => void,
  onHover: (event?: AircraftHoverEvent) => void
): AircraftInteractionController {
  return attachAircraftInteraction({
    canvas,
    camera,
    targets: fleet.interactionTargets,
    initialSelection: selectedId,
    onHover,
    onSelect,
  });
}

function formationSignature(formation: readonly FormationAircraft[]): string {
  return formation
    .map(
      (entry) =>
        `${entry.instanceId}:${entry.candidate}:${entry.candidateLabel}:${entry.model}:${entry.effort}:${entry.configuration.airframe.airframe.id}:${entry.configuration.loadout.id}:${entry.pilot.id}:${entry.position.join(',')}:${entry.scale}`
    )
    .join('|');
}

function disposeMaterial(material: THREE.Material | THREE.Material[]): void {
  for (const item of Array.isArray(material) ? material : [material]) item.dispose();
}
