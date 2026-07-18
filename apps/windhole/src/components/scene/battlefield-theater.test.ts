import { describe, expect, it } from 'vitest';
import { BATTLEFIELD_THEATERS, taskTheater } from './battlefield-theater';

describe('task battlefield theater', () => {
  it.each([
    ['quick_file_edit', 'training-range'],
    ['ann_vector_search_qps', 'littoral-front'],
    ['bipedalwalker_locomotion_rl', 'mountain-pass'],
    ['portfolio_risk_calibration', 'desert-frontier'],
    ['rust_multicrate_reconstruction', 'arctic-highland'],
    ['warehouse_forklift_routing', 'industrial-city'],
    ['college_english_exam_bank', 'forest-valley'],
    ['wireless_electricity_layout', 'ocean-platforms'],
  ] as const)('maps %s to the %s exterior', (taskId, theaterId) => {
    expect(taskTheater(taskId)).toBe(BATTLEFIELD_THEATERS[theaterId]);
  });

  it('gives all demo Tasks visibly different profile identities', () => {
    const profileIds = [
      'quick_file_edit',
      'ann_vector_search_qps',
      'bipedalwalker_locomotion_rl',
      'portfolio_risk_calibration',
      'rust_multicrate_reconstruction',
      'warehouse_forklift_routing',
      'college_english_exam_bank',
      'wireless_electricity_layout',
    ].map((taskId) => taskTheater(taskId).id);

    expect(new Set(profileIds).size).toBe(8);
    expect(Object.values(BATTLEFIELD_THEATERS).every((profile) => profile.labelZh.length > 0)).toBe(true);
  });

  it('uses category hints when available and a stable fallback otherwise', () => {
    expect(taskTheater('future_systems_task', 'Systems & Software Engineering')).toBe(
      BATTLEFIELD_THEATERS['industrial-city']
    );
    expect(taskTheater('future_agent_task')).toBe(taskTheater('future_agent_task'));
    expect(Object.values(BATTLEFIELD_THEATERS)).toContain(taskTheater('future_agent_task'));
  });
});
