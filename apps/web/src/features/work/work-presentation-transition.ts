import type { WorkSlideTransition, WorkSlideTransitionDirection, WorkSlideTransitionType } from './work-types';

const CARDINAL_DIRECTIONS = new Set<WorkSlideTransitionDirection>(['left', 'right', 'up', 'down']);
const SPLIT_DIRECTIONS = new Set<WorkSlideTransitionDirection>(['in', 'out']);

export function createWorkSlideTransition(
  type: WorkSlideTransitionType,
  previous?: WorkSlideTransition
): WorkSlideTransition {
  const common = {
    type,
    speed: previous?.speed ?? ('medium' as const),
    advanceOnClick: previous?.advanceOnClick ?? true,
    advanceAfterMs: previous?.advanceAfterMs,
  };
  if (type === 'push' || type === 'wipe') {
    return {
      ...common,
      direction: previous?.direction && CARDINAL_DIRECTIONS.has(previous.direction) ? previous.direction : 'left',
    };
  }
  if (type === 'split') {
    return {
      ...common,
      direction: previous?.direction && SPLIT_DIRECTIONS.has(previous.direction) ? previous.direction : 'out',
      orientation: previous?.orientation ?? 'horizontal',
    };
  }
  return common;
}

export function slideTransitionDurationMilliseconds(transition: WorkSlideTransition | undefined): number {
  if (transition?.speed === 'fast') return 500;
  if (transition?.speed === 'slow') return 2000;
  return 1000;
}
