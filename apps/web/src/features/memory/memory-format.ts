import type { MemoryForgetSignal, MemoryTier } from '../../types/api';

const memoryTypeLabels: Record<string, string> = {
  episodic: '经历',
  semantic: '知识',
  procedural: '做法',
  working: '当前任务',
  memory: '记忆',
};

const entityKindLabels: Record<string, string> = {
  source: '来源',
  tag: '标签',
  provider: '服务',
  session: '对话',
  'ctx-event': '对话',
  date: '日期',
  prompt: '提问',
  error: '错误',
  tool: '工具',
  topic: '主题',
  outcome: '结果',
  url: '链接',
  command: '命令',
  file: '文件',
};

const memorySourceLabels: Record<string, string> = {
  success: '完成的任务',
  failure: '问题记录',
  project_fact: '项目信息',
  workflow: '任务记录',
  decision: '决定',
  preference: '偏好',
  sleep: '自动整理',
  consolidation: '自动整理',
  llm_extractor: 'A3S 保存',
  ctx: '对话',
  memory: '手动记录',
};

const hiddenMemoryTags = new Set([
  'llm',
  'extracted',
  'consolidated',
  'conflict',
  'keep',
  'pinned',
  'protected',
  'success',
  'failure',
  'project_fact',
  'workflow',
  'decision',
  'preference',
  'sleep',
]);

const memoryTagLabels: Record<string, string> = {
  language_preference: '语言偏好',
  'user-preference': '用户偏好',
  user_preference: '用户偏好',
  creative_writing: '创意写作',
};

const systemTagEntityLabels: Record<string, string> = {
  llm: 'A3S 保存',
  extracted: 'A3S 保存',
  consolidated: '已去重',
  conflict: '待处理冲突',
  keep: '正常保留',
  pinned: '重点保留',
  protected: '重点保留',
};

export function memoryTypeLabel(type: string): string {
  return memoryTypeLabels[type] ?? '其他';
}

export function entityKindLabel(kind: string): string {
  return entityKindLabels[kind] ?? '相关内容';
}

export function memorySourceLabel(source: string): string {
  return memorySourceLabels[source] ?? '相关记录';
}

export function memoryTagLabel(tag: string): string | null {
  if (hiddenMemoryTags.has(tag)) return null;
  return memoryTagLabels[tag] ?? tag;
}

export function entityNameLabel(kind: string, name: string): string {
  if (kind === 'source') return memorySourceLabel(name);
  if (kind === 'outcome' && (name === 'success' || name === 'failure')) return memorySourceLabel(name);
  if (kind === 'tag') {
    return memoryTagLabels[name] ?? systemTagEntityLabels[name] ?? memorySourceLabels[name] ?? name;
  }
  return name;
}

export function tierLabel(tier: MemoryTier): string {
  if (tier === 'short') return '短期';
  if (tier === 'mid') return '中期';
  return '长期';
}

export function forgetSignalLabel(signal: MemoryForgetSignal): string {
  if (signal === 'protected') return '重点保留';
  if (signal === 'candidate') return '可清理';
  if (signal === 'cooling') return '观察中';
  return '正常保留';
}

export function formatMemoryDate(value?: string | null): string {
  const timestamp = value ? new Date(value) : null;
  if (!timestamp || Number.isNaN(timestamp.getTime())) return '时间未知';
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(timestamp);
}

export function relativeMemoryTime(value?: string | null, now = Date.now()): string {
  const timestamp = value ? Date.parse(value) : Number.NaN;
  if (!Number.isFinite(timestamp)) return '时间未知';
  const seconds = Math.max(0, Math.round((now - timestamp) / 1000));
  if (seconds < 60) return '刚刚';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days} 天前`;
  const months = Math.floor(days / 30);
  if (months < 12) return `${months} 个月前`;
  return `${Math.floor(months / 12)} 年前`;
}

export function memoryDayLabel(value: string, now = new Date()): string {
  const timestamp = new Date(value);
  if (Number.isNaN(timestamp.getTime())) return '时间未知';
  const today = dayKey(now);
  const target = dayKey(timestamp);
  const difference = Math.round((Date.parse(today) - Date.parse(target)) / 86_400_000);
  if (difference === 0) return '今天';
  if (difference === 1) return '昨天';
  return new Intl.DateTimeFormat('zh-CN', {
    year: timestamp.getFullYear() === now.getFullYear() ? undefined : 'numeric',
    month: 'long',
    day: 'numeric',
  }).format(timestamp);
}

export function percent(value: number): string {
  return `${Math.round(Math.min(1, Math.max(0, value)) * 100)}%`;
}

function dayKey(value: Date): string {
  return `${value.getFullYear()}-${String(value.getMonth() + 1).padStart(2, '0')}-${String(value.getDate()).padStart(
    2,
    '0'
  )}T00:00:00`;
}
