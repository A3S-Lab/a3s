import type { BenchDoctorResult, BenchTask } from '../../types/bench';

export const RESTORED_TASK_MISSING_REASON = 'restored_snapshot_not_in_current_bench_catalog';

export interface TaskDeploymentStatus {
  deployable: boolean;
  message: string;
}

export function taskDeploymentStatus(
  task: Readonly<BenchTask>,
  doctor: Readonly<BenchDoctorResult> | undefined
): TaskDeploymentStatus {
  if (task.availability !== 'ready') {
    if (task.availability_reason === RESTORED_TASK_MISSING_REASON) {
      return {
        deployable: false,
        message: '该地图来自已恢复的评测，但当前 Bench 任务目录未返回它；仅可用于场景与归属展示。',
      };
    }
    return { deployable: false, message: task.availability_reason || '该任务当前不可运行。' };
  }
  if (task.availability_reason === 'requires_configured_judge_model' && !doctor?.judge_model?.trim()) {
    return {
      deployable: false,
      message: '该地图需要 Judge 模型；请先在 Bench 配置中设置 judge_model。',
    };
  }
  return { deployable: true, message: '地图输入将在部署时由 Bench 解析并锁定。' };
}
