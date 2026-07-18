import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { DEFAULT_HANGAR_ROSTER } from '../../features/hangar/hangar-configuration';
import { defaultTunnelParameters } from '../../state/lab-state';
import { applyAircraftTaskPose, createAircraftFleet, updateAircraftFleet } from './aircraft-fleet';
import { buildRosterFormation } from './flight-formation';

describe('aircraft fleet', () => {
  it('creates a separate transform stack for every Candidate', () => {
    const fleet = createAircraftFleet(buildDefaultFormation());

    expect(fleet.instances).toHaveLength(3);
    expect(new Set(fleet.instances.map((instance) => instance.model)).size).toBe(3);
    for (const instance of fleet.instances) {
      expect(instance.model.parent).toBe(instance.inspectionPivot);
      expect(instance.inspectionPivot.parent).toBe(instance.taskPoseRoot);
      expect(instance.taskPoseRoot.parent).toBe(instance.aeroRoot);
      expect(instance.aeroRoot.parent).toBe(instance.laneRoot);
    }
  });

  it('keeps aerodynamic attitude separate from the user inspection rotation', () => {
    const fleet = createAircraftFleet(buildDefaultFormation());
    const selected = fleet.instances[0];
    selected.inspectionPivot.rotation.set(0.2, 0.4, 0);

    updateAircraftFleet(fleet, { ...defaultTunnelParameters, angleOfAttack: 12 }, 1, 1);

    expect(selected.aeroRoot.rotation.z).toBeGreaterThan(0);
    expect(selected.inspectionPivot.rotation.x).toBeCloseTo(0.2);
    expect(selected.inspectionPivot.rotation.y).toBeCloseTo(0.4);
  });

  it('resets manual inspection and transitions to a task-specific attitude', () => {
    const fleet = createAircraftFleet(buildDefaultFormation());
    const selected = fleet.instances[0];
    selected.inspectionPivot.rotation.set(0.2, 0.4, -0.1);

    applyAircraftTaskPose(fleet, 'quick_file_edit');
    const firstTarget = selected.taskPoseTarget.clone();

    expect(selected.inspectionPivot.rotation.toArray().slice(0, 3)).toEqual([0, 0, 0]);
    expect(firstTarget.length()).toBeGreaterThan(0);

    applyAircraftTaskPose(fleet, 'rust_multicrate_reconstruction');
    expect(selected.taskPoseTarget.equals(firstTarget)).toBe(false);

    updateAircraftFleet(fleet, defaultTunnelParameters, 1, 1);
    expect(Math.abs(selected.taskPoseRoot.rotation.x)).toBeGreaterThan(0);
  });

  it('exposes the selected aircraft through raycast metadata', () => {
    const fleet = createAircraftFleet(buildDefaultFormation());
    const fuselage = fleet.instances[1].model.getObjectByName('fuselage');

    expect(fuselage?.userData.aircraftId).toBe(DEFAULT_HANGAR_ROSTER[1].id);
    expect(fleet.centers.every((center) => center instanceof THREE.Vector3)).toBe(true);
  });

  it('mounts the saved roster airframe, pilot, and effort loadout together', () => {
    const fleet = createAircraftFleet(buildDefaultFormation());
    const configured = fleet.instances[0].model;

    expect(configured.name).toBe('aircraft:j-50');
    expect(configured.getObjectByName('pilot:a3s')).toBeInstanceOf(THREE.Group);
    expect(configured.getObjectByName('weapon-loadout:heavy-air-to-air')).toBeInstanceOf(THREE.Group);
    expect(configured.userData.specimen).toMatchObject({ effort: 'high', airframe: { id: 'j-50' } });
  });
});

function buildDefaultFormation() {
  return buildRosterFormation(DEFAULT_HANGAR_ROSTER);
}
