export const WORK_AGENT_PROPOSAL_PROTOCOL = 'a3s-work-proposal/v1';

const maximumProposalTargets = 200;
const maximumTargetSourceLength = 12_000;
const maximumTargetManifestLength = 24_000;
const maximumReplacementLength = 20_000;

export interface WorkAgentProposalTarget {
  id: string;
  label: string;
  before: string;
}

export interface WorkAgentProposalChange extends WorkAgentProposalTarget {
  after: string;
  reason: string;
}

export interface WorkAgentProposalConflict {
  targetId: string;
  label: string;
  message: string;
}

export interface WorkAgentProposalApplyResult {
  appliedTargetIds: string[];
  conflicts: WorkAgentProposalConflict[];
}

export interface WorkAgentProposalRequest {
  id: string;
  title: string;
  description: string;
  targets: WorkAgentProposalTarget[];
  apply: (changes: readonly WorkAgentProposalChange[]) => WorkAgentProposalApplyResult;
}

export interface WorkAgentProposal {
  requestId: string;
  summary: string;
  changes: WorkAgentProposalChange[];
}

export interface WorkAgentProposalMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
  pending?: boolean;
}

export type WorkAgentProposalStatus =
  | { state: 'waiting'; phase: 'draft' | 'response' }
  | { state: 'invalid'; message: string }
  | { state: 'ready'; proposal: WorkAgentProposal };

export function createWorkAgentProposalRequest({
  id = createProposalId(),
  title,
  description,
  targets,
  apply,
}: {
  id?: string;
  title: string;
  description: string;
  targets: readonly WorkAgentProposalTarget[];
  apply: WorkAgentProposalRequest['apply'];
}): WorkAgentProposalRequest {
  const uniqueTargets = new Map<string, WorkAgentProposalTarget>();
  let manifestLength = 0;
  for (const target of targets) {
    const targetId = target.id.trim();
    const label = target.label.trim() || targetId;
    const targetLength = targetId.length + label.length + target.before.length;
    if (
      !targetId ||
      uniqueTargets.has(targetId) ||
      uniqueTargets.size >= maximumProposalTargets ||
      target.before.length > maximumTargetSourceLength ||
      manifestLength + targetLength > maximumTargetManifestLength
    ) {
      continue;
    }
    uniqueTargets.set(targetId, {
      id: targetId,
      label,
      before: target.before,
    });
    manifestLength += targetLength;
  }
  return {
    id,
    title: title.trim() || 'AI 修改建议',
    description: description.trim(),
    targets: [...uniqueTargets.values()],
    apply,
  };
}

export function workAgentProposalInstruction(instruction: string, request: WorkAgentProposalRequest): string {
  if (!request.targets.length) return instruction.trim();
  const manifest = request.targets.map((target) => ({
    targetId: target.id,
    label: target.label,
    before: target.before,
  }));
  return [
    instruction.trim(),
    '',
    '请先给出简短说明，再在回复末尾提供一份可由 A3S Work 审阅的结构化修改建议。不要调用工具或直接修改文件。',
    `请求 ID：${request.id}`,
    '只能使用下面清单中的 targetId；不需要改动的目标不要出现在 changes 中：',
    JSON.stringify(manifest, null, 2),
    '',
    '回复末尾必须包含且只包含一个符合以下形状的 JSON 代码块：',
    '```json',
    JSON.stringify(
      {
        protocol: WORK_AGENT_PROPOSAL_PROTOCOL,
        requestId: request.id,
        summary: '一句话概括建议',
        changes: [
          {
            targetId: manifest[0]?.targetId ?? 'target',
            after: '建议的新内容',
            reason: '修改理由',
          },
        ],
      },
      null,
      2
    ),
    '```',
    'after 必须是完整替换内容；若要清空目标，请使用空字符串。不要虚构清单之外的 targetId。',
  ].join('\n');
}

export function workAgentProposalStatus(
  messages: readonly WorkAgentProposalMessage[],
  request: WorkAgentProposalRequest
): WorkAgentProposalStatus {
  let requestIndex = -1;
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message.role === 'user' && message.content.includes(request.id)) {
      requestIndex = index;
      break;
    }
  }
  if (requestIndex < 0) return { state: 'waiting', phase: 'draft' };

  const responses = messages.slice(requestIndex + 1).filter((message) => message.role === 'assistant');
  for (let index = responses.length - 1; index >= 0; index -= 1) {
    const response = responses[index];
    if (response.pending) continue;
    const parsed = parseProposalResponse(response.content, request);
    if (parsed) return parsed;
  }
  if (responses.some((response) => response.pending)) return { state: 'waiting', phase: 'response' };
  if (!responses.length) return { state: 'waiting', phase: 'response' };
  return {
    state: 'invalid',
    message: 'AI 助手已返回说明，但没有生成可验证的 A3S Work 差异。你可以调整草稿后重试。',
  };
}

function parseProposalResponse(
  content: string,
  request: WorkAgentProposalRequest
): Extract<WorkAgentProposalStatus, { state: 'ready' }> | null {
  const targetById = new Map(request.targets.map((target) => [target.id, target]));
  for (const candidate of proposalJsonCandidates(content)) {
    let value: unknown;
    try {
      value = JSON.parse(candidate);
    } catch {
      continue;
    }
    if (!isRecord(value)) continue;
    if (value.protocol !== WORK_AGENT_PROPOSAL_PROTOCOL || value.requestId !== request.id) continue;
    if (!Array.isArray(value.changes)) continue;

    const changes: WorkAgentProposalChange[] = [];
    const seen = new Set<string>();
    let valid = true;
    for (const rawChange of value.changes.slice(0, maximumProposalTargets)) {
      if (!isRecord(rawChange)) {
        valid = false;
        break;
      }
      const targetId = typeof rawChange.targetId === 'string' ? rawChange.targetId.trim() : '';
      const target = targetById.get(targetId);
      if (!target || seen.has(targetId) || typeof rawChange.after !== 'string') {
        valid = false;
        break;
      }
      if (rawChange.after.length > maximumReplacementLength) {
        valid = false;
        break;
      }
      seen.add(targetId);
      if (rawChange.after === target.before) continue;
      changes.push({
        ...target,
        after: rawChange.after,
        reason: typeof rawChange.reason === 'string' ? rawChange.reason.trim().slice(0, 2_000) : '',
      });
    }
    if (!valid || !changes.length) continue;
    return {
      state: 'ready',
      proposal: {
        requestId: request.id,
        summary:
          typeof value.summary === 'string' && value.summary.trim()
            ? value.summary.trim().slice(0, 2_000)
            : request.title,
        changes,
      },
    };
  }
  return null;
}

function proposalJsonCandidates(content: string): string[] {
  const candidates: string[] = [];
  for (const match of content.matchAll(/```(?:json)?\s*([\s\S]*?)```/gi)) {
    if (match[1]?.trim()) candidates.push(match[1].trim());
  }
  const firstBrace = content.indexOf('{');
  const lastBrace = content.lastIndexOf('}');
  if (firstBrace >= 0 && lastBrace > firstBrace) candidates.push(content.slice(firstBrace, lastBrace + 1));
  return candidates;
}

function createProposalId(): string {
  const id = globalThis.crypto?.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
  return `work-proposal-${id}`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}
