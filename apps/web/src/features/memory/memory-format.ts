import type { MemoryForgetSignal, MemoryTier } from '../../types/api';

const memoryTypeLabels: Record<string, string> = {
  episodic: '经历',
  semantic: '知识',
  procedural: '流程',
  working: '工作',
  memory: '记忆',
};

const entityKindLabels: Record<string, string> = {
  source: '来源',
  tag: '标签',
  provider: '提供方',
  session: '会话',
  'ctx-event': '上下文事件',
  date: '日期',
  prompt: '提示',
  error: '错误',
  tool: '工具',
  topic: '主题',
  outcome: '结果',
  url: '链接',
  command: '命令',
  file: '文件',
};

const memorySourceLabels: Record<string, string> = {
  success: '成功任务',
  failure: '失败任务',
  project_fact: '项目事实',
  workflow: '工作流程',
  decision: '决策',
  preference: '用户偏好',
  sleep: '整理归并',
  consolidation: '整理归并',
  llm_extractor: '自动提取',
  ctx: '上下文记录',
  memory: '记忆记录',
};

const relationKindLabels: Record<string, string> = {
  from: '来源',
  via: '来源',
  tagged: '标签',
  touches: '涉及文件',
  used: '使用工具',
  'co-occurs': '共同出现',
  aliases: '别名',
  'resulted-in': '产生结果',
  'in-session': '所属会话',
  references: '引用',
  mentions: '提及',
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
  llm: '自动提取',
  extracted: '提取标记',
  consolidated: '已合并重复',
  conflict: '待处理冲突',
  keep: '正常保留',
  pinned: '重点保留',
  protected: '重点保留',
};

export function memoryTypeLabel(type: string): string {
  return memoryTypeLabels[type] ?? type;
}

export function entityKindLabel(kind: string): string {
  return entityKindLabels[kind] ?? kind;
}

export function memorySourceLabel(source: string): string {
  return memorySourceLabels[source] ?? source;
}

export function relationKindLabel(kind: string): string {
  return relationKindLabels[kind] ?? kind;
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
  if (signal === 'candidate') return '建议清理';
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
