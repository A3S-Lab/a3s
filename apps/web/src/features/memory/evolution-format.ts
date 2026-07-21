import type { EvolutionAuditEvent, EvolutionCandidateState, EvolutionKind } from '../../types/api';

export function formatEvolutionDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date);
}

export function evolutionKindLabel(kind: EvolutionKind): string {
  return { preference: '偏好', skill: '做事方法', okf: '项目知识' }[kind];
}

export function evolutionStateLabel(state: EvolutionCandidateState): string {
  return {
    observing: '学习中',
    ready: '待确认',
    materialized: '已保存',
    rejected: '已忽略',
    rolledBack: '已恢复',
  }[state];
}

export function evolutionSourceLabel(source: string): string {
  return (
    {
      preference: '你的偏好',
      workflow: '任务记录',
      failure: '问题记录',
      project_fact: '项目信息',
      decision: '决定',
      task: '任务记录',
      assistant: 'A3S 判断',
      memory: '记忆',
    }[source] ?? '相关记录'
  );
}

export function evolutionAuditLabel(action: EvolutionAuditEvent['action']): string {
  return {
    ready: '可以确认',
    materialized: '已保存',
    updated: '已更新',
    rejected: '已忽略',
    reopened: '重新考虑',
    rolledBack: '已恢复旧版本',
    activated: '已在对话中使用',
    deactivated: '已从对话中移除',
  }[action];
}
