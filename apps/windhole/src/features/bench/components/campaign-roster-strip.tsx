import type { BenchCampaignMemberStatus, BenchRunResult, BenchRunStage } from '../../../types/bench';
import { candidateRunStatus, type HangarRosterEntry } from '../../hangar/hangar-configuration';

export interface CampaignRosterMemberView {
  readonly rosterEntryId: string;
  readonly status: BenchCampaignMemberStatus;
  readonly stage?: BenchRunStage;
  readonly result?: Readonly<BenchRunResult>;
  readonly error?: string;
}

export interface CampaignRosterStripProps {
  roster: readonly Readonly<HangarRosterEntry>[];
  campaignMembers?: readonly CampaignRosterMemberView[];
  activeEntryId?: string;
  disabled?: boolean;
  onSelectEntry: (entryId: string) => void;
}

type CampaignRosterVisualStatus = BenchCampaignMemberStatus | 'ready' | 'blocked';

interface RosterStatusProjection {
  visualStatus: CampaignRosterVisualStatus;
  label: string;
  detail: string;
  score?: string;
}

const MEMBER_STATUS_LABELS = {
  queued: 'QUEUED',
  starting: 'STARTING',
  running: 'RUNNING',
  completed: 'COMPLETED',
  failed: 'FAILED',
  tracking_stopped: 'TRACKING STOPPED',
} as const satisfies Record<BenchCampaignMemberStatus, string>;

export function CampaignRosterStrip({
  roster,
  campaignMembers = [],
  activeEntryId,
  disabled = false,
  onSelectEntry,
}: CampaignRosterStripProps) {
  const membersByRosterEntryId = new Map(campaignMembers.map((member) => [member.rosterEntryId, member]));

  return (
    <section className='campaign-roster-strip' aria-label='编队出击状态'>
      <ol>
        {roster.map((entry, index) => {
          const member = membersByRosterEntryId.get(entry.id);
          const status = member ? campaignMemberStatus(member) : rosterReadiness(entry);
          const active = entry.id === activeEntryId;
          const callsign = entry.callsign.trim() || entry.id;
          const accessibleStatus = status.score
            ? `${status.label}，评分 ${status.score}`
            : `${status.label}，${status.detail}`;

          return (
            <li key={entry.id} data-status={status.visualStatus} className={active ? 'is-active' : undefined}>
              <button
                type='button'
                aria-label={`选择 ${callsign}，${accessibleStatus}`}
                aria-pressed={active}
                disabled={disabled}
                title={status.detail}
                onClick={() => onSelectEntry(entry.id)}
              >
                <span className='campaign-roster-strip__index'>{String(index + 1).padStart(2, '0')}</span>
                <span className='campaign-roster-strip__identity'>
                  <strong>{callsign}</strong>
                  <small>{status.detail}</small>
                </span>
                <span className='campaign-roster-strip__status'>
                  <i aria-hidden='true' />
                  {status.label}
                </span>
                {status.score ? (
                  <output className='campaign-roster-strip__score' aria-label={`${callsign} 真实评分`}>
                    {status.score}
                  </output>
                ) : null}
              </button>
            </li>
          );
        })}
      </ol>
    </section>
  );
}

function rosterReadiness(entry: Readonly<HangarRosterEntry>): RosterStatusProjection {
  const readiness = candidateRunStatus(entry.candidate, entry.model);
  return readiness.deployable
    ? {
        visualStatus: 'ready',
        label: 'READY',
        detail: readiness.message,
      }
    : {
        visualStatus: 'blocked',
        label: 'NEEDS CONFIG',
        detail: readiness.message,
      };
}

function campaignMemberStatus(member: CampaignRosterMemberView): RosterStatusProjection {
  const base = {
    visualStatus: member.status,
    label: MEMBER_STATUS_LABELS[member.status],
  } as const;

  switch (member.status) {
    case 'queued':
      return { ...base, detail: '等待跑道' };
    case 'starting':
      return { ...base, detail: '正在提交到 Bench' };
    case 'running':
      return { ...base, detail: member.stage ? benchStageLabel(member.stage) : 'Bench 正在执行' };
    case 'completed': {
      const score = member.result?.score?.trim();
      return {
        ...base,
        detail: score ? `Bench 返回评分 ${score}` : 'Bench 已返回战报，但未返回评分',
        score: score || undefined,
      };
    }
    case 'failed':
      return { ...base, detail: member.error?.trim() || 'Bench 评测失败' };
    case 'tracking_stopped':
      return { ...base, detail: '前端跟踪已停止，Bench Job 可能仍在运行' };
  }
}

function benchStageLabel(stage: BenchRunStage): string {
  return stage
    .split('_')
    .map((part) => part.toUpperCase())
    .join(' ');
}
