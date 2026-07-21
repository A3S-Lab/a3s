import * as THREE from 'three';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { attachAircraftInteraction, type AircraftInteractionTarget } from './aircraft-interaction';

describe('aircraft interaction selection', () => {
  const controllers: Array<ReturnType<typeof attachAircraftInteraction>> = [];

  afterEach(() => {
    for (const controller of controllers.splice(0)) controller.dispose();
  });

  it('keeps programmatic selection silent and reports keyboard activation', () => {
    const canvas = document.createElement('canvas');
    const onSelect = vi.fn();
    const targets = [interactionTarget('lead'), interactionTarget('wing')];
    const controller = attachAircraftInteraction({
      canvas,
      camera: new THREE.PerspectiveCamera(),
      targets,
      initialSelection: 'lead',
      onSelect,
      onHover: vi.fn(),
    });
    controllers.push(controller);

    controller.syncSelection('wing');
    expect(onSelect).not.toHaveBeenCalled();

    canvas.dispatchEvent(new KeyboardEvent('keydown', { key: '1', bubbles: true, cancelable: true }));
    expect(onSelect).toHaveBeenCalledOnce();
    expect(onSelect).toHaveBeenCalledWith('lead');
  });

  it('ignores an unknown programmatic selection without emitting', () => {
    const canvas = document.createElement('canvas');
    const onSelect = vi.fn();
    const controller = attachAircraftInteraction({
      canvas,
      camera: new THREE.PerspectiveCamera(),
      targets: [interactionTarget('lead')],
      initialSelection: 'lead',
      onSelect,
      onHover: vi.fn(),
    });
    controllers.push(controller);

    controller.syncSelection('missing');
    expect(onSelect).not.toHaveBeenCalled();
  });
});

function interactionTarget(id: string): AircraftInteractionTarget {
  return {
    id,
    hitRoot: new THREE.Group(),
    inspectionPivot: new THREE.Group(),
  };
}
