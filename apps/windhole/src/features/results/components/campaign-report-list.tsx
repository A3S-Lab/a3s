import { ChevronRight, Plane, Radio } from 'lucide-react';
import type { BenchCampaignMemberStatus, BenchCampaignStatus, BenchRunStage } from '../../../types/bench';
import { HANGAR_AIRFRAME_OPTIONS } from '../../hangar/hangar-configuration';

export interface CampaignReportRosterSortie {
  readonly rosterEntry: {
    readonly id: string;
    readonly callsign: string;
    readonly airframeId: string;
  };
}

export interface CampaignReportMember {
  readonly rosterEntryId: string;
  readonly status: BenchCampaignMemberStatus;
  readonly stage?: BenchRunStage;
  readonly runId?: string;
  readonly result?: {
    readonly status: BenchRunStage;
    readonly run_id: string;
    readonly score?: string;
  };
  readonly error?: string;
}

interface CampaignReportListProps {
  roster: readonly CampaignReportRosterSortie[];
  members: readonly CampaignReportMember[];
  status: BenchCampaignStatus;
  selectedRunId?: string;
  loading?: boolean;
  onOpenResult: (runId: string) => void;
}

const MEMBER_STATUS_LABELS = {
  queued: '等待跑道',
  starting: '正在提交',
  running: '评测中',
  completed: '已归档',
  failed: '行动失败',
  tracking_stopped: '跟踪已停止',
} as const satisfies Record<BenchCampaignMemberStatus, string>;

const CAMPAIGN_STATUS_LABELS = {
  idle: '未部署',
  running: '编队执行中',
  completed: '全员归档',
  completed_with_failures: '部分归档',
  failed: '编队失败',
  tracking_stopped: '跟踪已停止',
} as const satisfies Record<BenchCampaignStatus, string>;

export function CampaignReportList({
  roster,
  members,
  status,
  selectedRunId,
  loading = false,
  onOpenResult,
}: CampaignReportListProps) {
  if (roster.length === 0) return null;

  const membersByRosterEntryId = new Map(members.map((member) => [member.rosterEntryId, member]));
  const orderedMembers = roster.map((sortie) => membersByRosterEntryId.get(sortie.rosterEntry.id));
  const completed = orderedMembers.filter((member) => member?.status === 'completed').length;

  return (
    <section className='campaign-report-list' aria-labelledby='campaign-report-title'>
      <header className='campaign-report-list__header'>
        <Plane size={15} aria-hidden='true' />
        <span>
          <small>FORMATION DEBRIEF</small>
          <h2 id='campaign-report-title'>本次编队</h2>
        </span>
        <output aria-label='已归档编队成员'>
          {completed}/{roster.length}
        </output>
      </header>

      <p className='campaign-report-list__state'>
        <Radio size={11} aria-hidden='true' />
        {CAMPAIGN_STATUS_LABELS[status]}
      </p>

      <ol>
        {roster.map((sortie, index) => {
          const entry = sortie.rosterEntry;
          const member = membersByRosterEntryId.get(entry.id);
          const runId = member?.runId?.trim();
          const result = authoritativeResult(member, runId);
          const canOpen = Boolean(
            runId && member && ['completed', 'failed', 'tracking_stopped'].includes(member.status)
          );
          const airframe = HANGAR_AIRFRAME_OPTIONS.find((option) => option.id === entry.airframeId);
          const callsign = entry.callsign.trim() || entry.id;
          const statusLabel = member ? MEMBER_STATUS_LABELS[member.status] : '状态未记录';
          const detail = memberDetail(member, result?.score);

          return (
            <li key={entry.id} data-status={member?.status ?? 'missing'}>
              <button
                type='button'
                disabled={!canOpen || loading}
                aria-current={selectedRunId && runId === selectedRunId ? 'true' : undefined}
                aria-label={`${callsign}，${statusLabel}${canOpen ? '，按该机 Run ID 核验战报' : ''}`}
                onClick={() => {
                  if (canOpen && runId) onOpenResult(runId);
                }}
              >
                <span className='campaign-report-list__index'>{String(index + 1).padStart(2, '0')}</span>
                <span className='campaign-report-list__identity'>
                  <strong>{callsign}</strong>
                  <small>{airframe?.displayName ?? entry.airframeId}</small>
                </span>
                <span className='campaign-report-list__status'>{statusLabel}</span>
                <code title={runId}>{runId ?? '尚未生成 Run ID'}</code>
                {result?.score ? (
                  <output aria-label={`${callsign} 真实评分`}>{result.score}</output>
                ) : (
                  <small className={member?.status === 'failed' ? 'is-error' : undefined}>{detail}</small>
                )}
                {canOpen ? <ChevronRight size={13} aria-hidden='true' /> : null}
              </button>
            </li>
          );
        })}
      </ol>

      <footer>每架飞机仅按自身 Run ID 调取 Bench 公开战报。</footer>
    </section>
  );
}

function authoritativeResult(member: CampaignReportMember | undefined, runId: string | undefined) {
  if (
    member?.status !== 'completed' ||
    !runId ||
    member.result?.status !== 'completed' ||
    member.result.run_id !== runId
  ) {
    return undefined;
  }
  const score = member.result.score?.trim();
  return { score: score || undefined };
}

function memberDetail(member: CampaignReportMember | undefined, score: string | undefined): string {
  if (!member) return '缺少该成员的 Campaign 记录';
  if (member.error?.trim()) return member.error.trim();
  if (member.status === 'completed') return score ? `Bench 评分 ${score}` : '公开战报未返回评分';
  if (member.status === 'running' && member.stage) return benchStageLabel(member.stage);
  if (member.status === 'queued') return '等待可用并发席位';
  if (member.status === 'starting') return '正在创建 Bench Job';
  if (member.status === 'tracking_stopped') {
    return member.runId ? '已有 Run ID，可按该 ID 核验终态' : '已提交 Job 可能仍在运行';
  }
  return 'Bench 未返回详细错误';
}

function benchStageLabel(stage: BenchRunStage): string {
  const labels: Record<BenchRunStage, string> = {
    idle: '等待任务',
    planned: '等待部署',
    running: 'Bench 执行中',
    runtime_ready: '运行时就绪',
    inputs_resolved: '输入已解析',
    candidate_running: 'Candidate 执行中',
    candidate_completed: 'Candidate 已完成',
    judging: '裁定中',
    completed: '战报已归档',
    failed: '执行失败',
  };
  return labels[stage];
}
