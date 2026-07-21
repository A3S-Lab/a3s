import type { BenchDoctorResult, BenchTask } from '../../types/bench';
import { candidateRunStatus, type HangarRosterEntry } from '../hangar/hangar-configuration';
import { taskDeploymentStatus } from './task-deployment-status';

export interface CampaignDeploymentStatus {
  readonly deployable: boolean;
  readonly message: string;
  readonly blockedRosterEntryId?: string;
}

interface CampaignDeploymentStatusInput {
  readonly connectionMode: 'checking' | 'live' | 'preview';
  readonly doctor?: BenchDoctorResult;
  readonly locked: boolean;
  readonly roster: readonly Readonly<HangarRosterEntry>[];
  readonly task?: Readonly<BenchTask>;
}

/** Shared preflight for both the map UI and the real Campaign controller. */
export function campaignDeploymentStatus({
  connectionMode,
  doctor,
  locked,
  roster,
  task,
}: CampaignDeploymentStatusInput): CampaignDeploymentStatus {
  if (locked) {
    return {
      deployable: false,
      message: '编队评测仅支持普通 Candidate 模式；Lock 模式请使用单机评测。',
    };
  }
  if (connectionMode === 'checking') {
    return { deployable: false, message: 'Bench 连接与 Runtime 自检尚未完成，暂不能部署编队。' };
  }
  if (connectionMode !== 'live') {
    return { deployable: false, message: '当前仅可查看预览数据；连接可用的 A3S Bench 后才能部署编队。' };
  }
  if (!isDoctorReady(doctor)) {
    return { deployable: false, message: 'Bench Runtime Doctor 未就绪，不能部署编队。' };
  }
  if (!task) return { deployable: false, message: '请先选择作战地图。' };

  const taskStatus = taskDeploymentStatus(task, doctor);
  if (!taskStatus.deployable) return taskStatus;
  if (roster.length === 0) return { deployable: false, message: '机库编队为空，请先添加至少一架飞机。' };

  const invalidEntry = roster.find((entry) => !candidateRunStatus(entry.candidate, entry.model).deployable);
  if (invalidEntry) {
    const status = candidateRunStatus(invalidEntry.candidate, invalidEntry.model);
    return {
      deployable: false,
      message: `${invalidEntry.callsign} 无法部署：${status.message}`,
      blockedRosterEntryId: invalidEntry.id,
    };
  }

  return {
    deployable: true,
    message: `${roster.length} 架出击组合与当前地图已通过部署预检。`,
  };
}

function isDoctorReady(doctor: BenchDoctorResult | undefined): boolean {
  return (
    doctor?.runtime.ready === true &&
    doctor.runtime.provider.trim().length > 0 &&
    doctor.runtime.detail.trim().length > 0
  );
}
