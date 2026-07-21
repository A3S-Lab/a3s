import * as THREE from 'three';

export interface AircraftInteractionTarget {
  id: string;
  hitRoot: THREE.Object3D;
  inspectionPivot: THREE.Group;
}

export interface AircraftHoverEvent {
  id: string;
  x: number;
  y: number;
  placement: 'left' | 'right';
  hitPart: string;
}

interface AircraftInteractionOptions {
  canvas: HTMLCanvasElement;
  camera: THREE.Camera;
  targets: readonly AircraftInteractionTarget[];
  initialSelection: string;
  onSelect: (id: string) => void;
  onHover: (event?: AircraftHoverEvent) => void;
}

export interface AircraftInteractionController {
  syncSelection: (id: string) => void;
  reset: (id?: string) => void;
  dispose: () => void;
}

const MAX_PITCH = THREE.MathUtils.degToRad(38);

export function attachAircraftInteraction({
  canvas,
  camera,
  targets,
  initialSelection,
  onSelect,
  onHover,
}: AircraftInteractionOptions): AircraftInteractionController {
  const raycaster = new THREE.Raycaster();
  const pointer = new THREE.Vector2();
  const targetById = new Map(targets.map((target) => [target.id, target]));
  let selectedId = targetById.has(initialSelection) ? initialSelection : (targets[0]?.id ?? '');
  let hoveredId = '';
  let drag:
    | {
        pointerId: number;
        target: AircraftInteractionTarget;
        startX: number;
        startY: number;
        startPitch: number;
        startYaw: number;
      }
    | undefined;

  canvas.tabIndex = 0;
  canvas.setAttribute(
    'aria-label',
    'A3S智能体评测三维场景。点击选择飞机，拖动机身旋转观察，数字键切换飞机，方向键微调视角，按 R 或双击复位。'
  );

  const updateCursor = () => {
    canvas.dataset.interaction = drag ? 'grabbing' : hoveredId ? 'grab' : 'idle';
  };

  const hitTest = (
    event: PointerEvent | MouseEvent
  ): { target: AircraftInteractionTarget; hitPart: string } | undefined => {
    const rect = canvas.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return undefined;
    pointer.set(
      ((event.clientX - rect.left) / rect.width) * 2 - 1,
      -((event.clientY - rect.top) / rect.height) * 2 + 1
    );
    raycaster.setFromCamera(pointer, camera);
    const intersections = raycaster.intersectObjects(
      targets.map((target) => target.hitRoot),
      true
    );
    for (const intersection of intersections) {
      let object: THREE.Object3D | null = intersection.object;
      while (object) {
        const id = object.userData.aircraftId;
        if (typeof id === 'string') {
          const target = targetById.get(id);
          if (target) return { target, hitPart: intersection.object.name || 'airframe' };
        }
        object = object.parent;
      }
    }
    return undefined;
  };

  const syncSelection = (id: string): boolean => {
    if (!targetById.has(id)) return false;
    selectedId = id;
    return true;
  };

  const activate = (id: string) => {
    if (!syncSelection(id)) return;
    onSelect(id);
  };

  const reset = (id = selectedId) => {
    const target = targetById.get(id);
    if (!target) return;
    target.inspectionPivot.rotation.x = 0;
    target.inspectionPivot.rotation.y = 0;
  };

  const handlePointerDown = (event: PointerEvent) => {
    const hit = hitTest(event);
    if (!hit) return;
    const { target } = hit;
    event.preventDefault();
    canvas.focus({ preventScroll: true });
    activate(target.id);
    canvas.setPointerCapture(event.pointerId);
    drag = {
      pointerId: event.pointerId,
      target,
      startX: event.clientX,
      startY: event.clientY,
      startPitch: target.inspectionPivot.rotation.x,
      startYaw: target.inspectionPivot.rotation.y,
    };
    onHover(undefined);
    updateCursor();
  };

  const handlePointerMove = (event: PointerEvent) => {
    if (drag?.pointerId === event.pointerId) {
      event.preventDefault();
      const deltaX = event.clientX - drag.startX;
      const deltaY = event.clientY - drag.startY;
      drag.target.inspectionPivot.rotation.y = drag.startYaw + deltaX * 0.009;
      drag.target.inspectionPivot.rotation.x = THREE.MathUtils.clamp(
        drag.startPitch + deltaY * 0.007,
        -MAX_PITCH,
        MAX_PITCH
      );
      return;
    }
    const hit = hitTest(event);
    hoveredId = hit?.target.id ?? '';
    const rect = canvas.getBoundingClientRect();
    const localX = event.clientX - rect.left;
    const localY = event.clientY - rect.top;
    onHover(
      hit
        ? {
            id: hit.target.id,
            x: localX,
            y: THREE.MathUtils.clamp(
              localY,
              Math.min(170, rect.height / 2),
              Math.max(rect.height - 170, rect.height / 2)
            ),
            placement: localX > rect.width * 0.62 ? 'left' : 'right',
            hitPart: hit.hitPart,
          }
        : undefined
    );
    updateCursor();
  };

  const endDrag = (event: PointerEvent) => {
    if (drag?.pointerId !== event.pointerId) return;
    if (canvas.hasPointerCapture(event.pointerId)) canvas.releasePointerCapture(event.pointerId);
    drag = undefined;
    updateCursor();
  };

  const handleDoubleClick = (event: MouseEvent) => {
    const hit = hitTest(event);
    if (!hit) return;
    activate(hit.target.id);
    reset(hit.target.id);
  };

  const handleKeyDown = (event: KeyboardEvent) => {
    const aircraftIndex = Number(event.key) - 1;
    if (Number.isInteger(aircraftIndex) && targets[aircraftIndex]) {
      event.preventDefault();
      activate(targets[aircraftIndex].id);
      return;
    }
    const target = targetById.get(selectedId);
    if (!target) return;
    const step = THREE.MathUtils.degToRad(event.shiftKey ? 8 : 3);
    if (event.key.toLocaleLowerCase() === 'r') {
      event.preventDefault();
      reset();
      return;
    }
    if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
    event.preventDefault();
    if (event.key === 'ArrowLeft') target.inspectionPivot.rotation.y -= step;
    if (event.key === 'ArrowRight') target.inspectionPivot.rotation.y += step;
    if (event.key === 'ArrowUp') {
      target.inspectionPivot.rotation.x = Math.max(-MAX_PITCH, target.inspectionPivot.rotation.x - step);
    }
    if (event.key === 'ArrowDown') {
      target.inspectionPivot.rotation.x = Math.min(MAX_PITCH, target.inspectionPivot.rotation.x + step);
    }
  };

  const handlePointerLeave = () => {
    if (drag) return;
    hoveredId = '';
    onHover(undefined);
    updateCursor();
  };

  canvas.addEventListener('pointerdown', handlePointerDown);
  canvas.addEventListener('pointermove', handlePointerMove);
  canvas.addEventListener('pointerup', endDrag);
  canvas.addEventListener('pointercancel', endDrag);
  canvas.addEventListener('lostpointercapture', endDrag);
  canvas.addEventListener('pointerleave', handlePointerLeave);
  canvas.addEventListener('dblclick', handleDoubleClick);
  canvas.addEventListener('keydown', handleKeyDown);
  updateCursor();

  return {
    syncSelection,
    reset,
    dispose: () => {
      canvas.removeEventListener('pointerdown', handlePointerDown);
      canvas.removeEventListener('pointermove', handlePointerMove);
      canvas.removeEventListener('pointerup', endDrag);
      canvas.removeEventListener('pointercancel', endDrag);
      canvas.removeEventListener('lostpointercapture', endDrag);
      canvas.removeEventListener('pointerleave', handlePointerLeave);
      canvas.removeEventListener('dblclick', handleDoubleClick);
      canvas.removeEventListener('keydown', handleKeyDown);
      onHover(undefined);
      delete canvas.dataset.interaction;
    },
  };
}
