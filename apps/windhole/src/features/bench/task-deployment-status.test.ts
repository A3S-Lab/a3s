import { describe, expect, it } from 'vitest';
import { demoTasks } from '../../data/demo-tasks';
import { taskDeploymentStatus } from './task-deployment-status';

describe('task deployment status', () => {
  it('requires the real Judge model for maps that declare that dependency', () => {
    const task = { ...demoTasks[0], availability_reason: 'requires_configured_judge_model' };

    expect(taskDeploymentStatus(task, undefined)).toMatchObject({ deployable: false });
    expect(
      taskDeploymentStatus(task, { runtime: { provider: 'docker', ready: true, detail: 'ready' }, judge_model: null })
        .message
    ).toContain('Judge');
    expect(
      taskDeploymentStatus(task, {
        runtime: { provider: 'docker', ready: true, detail: 'ready' },
        judge_model: 'provider/judge',
      }).deployable
    ).toBe(true);
  });

  it('preserves the catalog block reason for unavailable maps', () => {
    const task = { ...demoTasks[0], availability: 'blocked' as const, availability_reason: 'runtime_not_supported' };

    expect(taskDeploymentStatus(task, undefined)).toEqual({
      deployable: false,
      message: 'runtime_not_supported',
    });
  });
});
