import type { BenchCampaignMemberStatus, BenchRunStage } from '../../types/bench';
import type { AircraftHudStatus } from '../aircraft-hud';

export interface CampaignAircraftHudMember {
  readonly rosterEntryId: string;
  readonly status: BenchCampaignMemberStatus;
  readonly stage?: BenchRunStage;
  readonly result?: {
    readonly score?: string;
  };
  readonly error?: string;
}

/** Projects one authoritative Campaign member into the HUD for its roster aircraft. */
export function campaignAircraftHudStatus(member: CampaignAircraftHudMember): AircraftHudStatus {
  switch (member.status) {
    case 'queued':
      return { label: 'QUEUED', tone: 'standby', detail: '等待跑道' };
    case 'starting':
      return { label: 'STARTING', tone: 'running', detail: '正在提交到 Bench' };
    case 'running':
      return {
        label: 'RUNNING',
        tone: 'running',
        detail: member.stage ? benchStageLabel(member.stage) : 'Bench 正在执行',
      };
    case 'completed': {
      const score = member.result?.score?.trim();
      return {
        label: 'COMPLETE',
        tone: 'success',
        detail: score ? `SCORE ${score}` : 'Bench 已完成，但战报未返回评分',
      };
    }
    case 'failed':
      return {
        label: 'FAILED',
        tone: 'error',
        detail: member.error?.trim() || 'Bench 未返回失败详情',
      };
    case 'tracking_stopped':
      return {
        label: 'TRACKING STOPPED',
        tone: 'warning',
        detail: '前端跟踪已停止，Bench Job 可能仍在运行',
      };
  }
}

function benchStageLabel(stage: BenchRunStage): string {
  return stage
    .split('_')
    .map((part) => part.toUpperCase())
    .join(' ');
}
