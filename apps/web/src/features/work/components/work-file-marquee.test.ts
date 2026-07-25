import { describe, expect, it } from 'vitest';
import {
  marqueeAutoScrollVelocity,
  marqueeSelectionPaths,
  rectangleBetweenPoints,
  rectanglesIntersect,
} from './work-file-marquee';

describe('Work file marquee selection', () => {
  it('normalizes a drag in every direction and detects item intersections', () => {
    const rectangle = rectangleBetweenPoints({ x: 180, y: 130 }, { x: 40, y: 30 });

    expect(rectangle).toEqual({
      left: 40,
      top: 30,
      right: 180,
      bottom: 130,
      width: 140,
      height: 100,
    });
    expect(
      rectanglesIntersect(rectangle, {
        left: 120,
        top: 80,
        right: 220,
        bottom: 160,
        width: 100,
        height: 80,
      })
    ).toBe(true);
    expect(
      rectanglesIntersect(rectangle, {
        left: 181,
        top: 30,
        right: 220,
        bottom: 80,
        width: 39,
        height: 50,
      })
    ).toBe(false);
  });

  it('replaces, adds, or toggles the initial selection in visible order', () => {
    const visiblePaths = ['/docs/a', '/docs/b', '/docs/c', '/docs/d'];
    const initialPaths = new Set(['/docs/a', '/docs/c']);
    const hitPaths = new Set(['/docs/b', '/docs/c']);

    expect(marqueeSelectionPaths(visiblePaths, initialPaths, hitPaths, 'replace')).toEqual(['/docs/b', '/docs/c']);
    expect(marqueeSelectionPaths(visiblePaths, initialPaths, hitPaths, 'add')).toEqual([
      '/docs/a',
      '/docs/b',
      '/docs/c',
    ]);
    expect(marqueeSelectionPaths(visiblePaths, initialPaths, hitPaths, 'toggle')).toEqual(['/docs/a', '/docs/b']);
  });

  it('accelerates auto-scroll near either viewport edge and stops in the safe center', () => {
    expect(marqueeAutoScrollVelocity(5, 0, 200)).toBeLessThan(-10);
    expect(marqueeAutoScrollVelocity(100, 0, 200)).toBe(0);
    expect(marqueeAutoScrollVelocity(195, 0, 200)).toBeGreaterThan(10);
    expect(marqueeAutoScrollVelocity(-20, 0, 200)).toBe(-18);
    expect(marqueeAutoScrollVelocity(240, 0, 200)).toBe(18);
  });
});
