import { ExternalLink, FileText, SearchCheck } from 'lucide-react';
import type { ToolCallProjection } from './tool-call-projection';

interface DeepResearchReport {
  status: 'synthesized' | 'qualified' | 'source_backed' | 'no_evidence';
  runId: string;
}

export function DeepResearchReportCard({
  calls,
  sessionId,
}: {
  calls: readonly ToolCallProjection[];
  sessionId: string;
}) {
  const report = calls
    .filter((call) => call.name === 'deep_research' && call.state === 'succeeded')
    .map(reportFromCall)
    .find((candidate): candidate is DeepResearchReport => Boolean(candidate));
  if (!report) return null;

  const htmlHref = researchArtifactHref(sessionId, report.runId, 'html');
  const markdownHref = researchArtifactHref(sessionId, report.runId, 'markdown');
  const status = reportStatus(report.status);

  return (
    <section className={`deep-research-report-card ${report.status}`} aria-label='DeepResearch 研究报告'>
      <span className='deep-research-report-icon' aria-hidden='true'>
        <SearchCheck size={18} />
      </span>
      <span className='deep-research-report-copy'>
        <strong>深度研究报告</strong>
        <small>{status}</small>
      </span>
      <span className='deep-research-report-actions'>
        <a href={markdownHref} target='_blank' rel='noopener noreferrer' aria-label='打开 Markdown 研究报告'>
          <FileText size={13} />
          Markdown
        </a>
        <a href={htmlHref} target='_blank' rel='noopener noreferrer' aria-label='打开网页版研究报告'>
          <ExternalLink size={13} />
          打开网页
        </a>
      </span>
    </section>
  );
}

function reportFromCall(call: ToolCallProjection): DeepResearchReport | null {
  const report = recordValue(call.metadata?.report);
  const status = stringValue(report?.status);
  const runId = stringValue(report?.runId);
  const artifactKinds = stringArray(report?.artifactKinds);
  if (!['synthesized', 'qualified', 'source_backed', 'no_evidence'].includes(status ?? '') || !runId) return null;
  if (!isRunId(runId) || !artifactKinds.includes('html') || !artifactKinds.includes('markdown')) return null;
  return { status: status as DeepResearchReport['status'], runId };
}

function researchArtifactHref(sessionId: string, runId: string, kind: 'html' | 'markdown'): string {
  return `/api/v1/kernel/sessions/${encodeURIComponent(sessionId)}/research-artifact?runId=${encodeURIComponent(
    runId
  )}&kind=${kind}`;
}

function isRunId(value: string): boolean {
  return value.length <= 128 && /^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(value);
}

function reportStatus(status: DeepResearchReport['status']): string {
  if (status === 'synthesized') return '综合报告已通过质量门槛';
  if (status === 'qualified') return '证据充分，建议复核限定条件';
  if (status === 'source_backed') return '已保留来源报告，综合结论未通过准入';
  return '未取得可安全发布的证据';
}

function recordValue(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : undefined;
}

function stringValue(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() ? value : undefined;
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];
}
