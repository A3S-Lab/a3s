import * as THREE from 'three';
import type { WindTunnelParameters } from '../../types/bench';
import { createAircraft } from './aircraft-registry';
import type { AircraftInteractionTarget } from './aircraft-interaction';
import type { FormationAircraft } from './flight-formation';
import { taskAircraftPose } from './task-aircraft-pose';

export interface AircraftInstance {
  descriptor: FormationAircraft;
  laneRoot: THREE.Group;
  aeroRoot: THREE.Group;
  taskPoseRoot: THREE.Group;
  taskPoseTarget: THREE.Vector3;
  inspectionPivot: THREE.Group;
  model: THREE.Group;
}

export interface AircraftFleet {
  group: THREE.Group;
  instances: AircraftInstance[];
  interactionTargets: AircraftInteractionTarget[];
  centers: THREE.Vector3[];
}

export function createAircraftFleet(formation: readonly FormationAircraft[]): AircraftFleet {
  const group = new THREE.Group();
  group.name = 'multi-candidate-aircraft-fleet';
  const instances = formation.map((descriptor) => createAircraftInstance(descriptor, group));
  return {
    group,
    instances,
    interactionTargets: instances.map((instance) => ({
      id: instance.descriptor.instanceId,
      hitRoot: instance.model,
      inspectionPivot: instance.inspectionPivot,
    })),
    centers: instances.map((instance) => instance.laneRoot.position),
  };
}

export function updateAircraftFleet(
  fleet: AircraftFleet,
  parameters: WindTunnelParameters,
  elapsed: number,
  motionScale: number
): void {
  const targetAngle = THREE.MathUtils.degToRad(parameters.angleOfAttack);
  for (const instance of fleet.instances) {
    instance.aeroRoot.rotation.z = THREE.MathUtils.lerp(instance.aeroRoot.rotation.z, targetAngle, 0.065);
    instance.taskPoseRoot.rotation.x = THREE.MathUtils.lerp(
      instance.taskPoseRoot.rotation.x,
      instance.taskPoseTarget.x,
      0.055
    );
    instance.taskPoseRoot.rotation.y = THREE.MathUtils.lerp(
      instance.taskPoseRoot.rotation.y,
      instance.taskPoseTarget.y,
      0.055
    );
    instance.taskPoseRoot.rotation.z = THREE.MathUtils.lerp(
      instance.taskPoseRoot.rotation.z,
      instance.taskPoseTarget.z,
      0.055
    );
    const baseY = instance.descriptor.position[1];
    instance.laneRoot.position.y = baseY + Math.sin(elapsed * 1.15 + instance.descriptor.phase) * 0.025 * motionScale;
  }
}

export function applyAircraftTaskPose(fleet: AircraftFleet, taskId: string, immediate = false): void {
  for (const [index, instance] of fleet.instances.entries()) {
    const pose = taskAircraftPose(taskId, index);
    instance.taskPoseTarget.set(pose.rollX, pose.yawY, pose.pitchZ);
    instance.inspectionPivot.rotation.set(0, 0, 0);
    if (immediate) instance.taskPoseRoot.rotation.set(pose.rollX, pose.yawY, pose.pitchZ);
  }
}

export function disposeAircraftFleet(fleet: AircraftFleet): void {
  fleet.group.traverse((object) => {
    if (object instanceof THREE.Mesh || object instanceof THREE.Line || object instanceof THREE.Points) {
      object.geometry.dispose();
      disposeMaterial(object.material);
    }
  });
}

function createAircraftInstance(descriptor: FormationAircraft, parent: THREE.Group): AircraftInstance {
  const laneRoot = new THREE.Group();
  laneRoot.name = `test-lane:${descriptor.instanceId}`;
  laneRoot.position.set(...descriptor.position);
  parent.add(laneRoot);

  const aeroRoot = new THREE.Group();
  aeroRoot.name = `attack-angle:${descriptor.instanceId}`;
  laneRoot.add(aeroRoot);

  const taskPoseRoot = new THREE.Group();
  taskPoseRoot.name = `task-pose:${descriptor.instanceId}`;
  aeroRoot.add(taskPoseRoot);

  const inspectionPivot = new THREE.Group();
  inspectionPivot.name = `inspection-pivot:${descriptor.instanceId}`;
  taskPoseRoot.add(inspectionPivot);

  const model = createAircraft(descriptor.candidate, {
    airframeId: descriptor.configuration.airframe.airframe.id,
    candidateFamily: descriptor.profile.candidateFamily,
    callsign: descriptor.candidateLabel,
    pilotProfile: descriptor.pilot,
    weaponLoadout: descriptor.configuration.loadout,
  });
  model.userData.specimen = {
    candidate: descriptor.candidate,
    candidateLabel: descriptor.candidateLabel,
    model: descriptor.model,
    effort: descriptor.effort,
    airframe: descriptor.configuration.airframe.airframe,
    loadout: descriptor.configuration.loadout,
    pilot: {
      id: descriptor.pilot.id,
      displayName: descriptor.pilot.displayName,
      helmetCode: descriptor.pilot.marking.helmetCode,
      attire: descriptor.pilot.marking.label,
    },
  };
  model.scale.setScalar(descriptor.scale);
  model.traverse((object) => {
    object.userData.aircraftId = descriptor.instanceId;
  });
  inspectionPivot.add(model);

  return {
    descriptor,
    laneRoot,
    aeroRoot,
    taskPoseRoot,
    taskPoseTarget: new THREE.Vector3(),
    inspectionPivot,
    model,
  };
}

function disposeMaterial(material: THREE.Material | THREE.Material[]): void {
  for (const item of Array.isArray(material) ? material : [material]) item.dispose();
}
