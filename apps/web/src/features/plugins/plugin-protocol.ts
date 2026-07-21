import type { PluginContextProposal } from './plugin-state';

export const activityProtocol = 'a3s.activity.v1';
const MAX_MESSAGE_CHARS = 32 * 1024;

export type PluginHostMessage =
  | { type: 'ready' }
  | { type: 'context'; proposal: PluginContextProposal }
  | { type: 'error'; message: string };

export function parsePluginMessage(value: unknown, sourceKey: string): PluginHostMessage | null {
  if (!isRecord(value)) return null;
  try {
    if (JSON.stringify(value).length > MAX_MESSAGE_CHARS) return null;
  } catch {
    return null;
  }
  if (value.protocol !== activityProtocol || typeof value.type !== 'string') return null;
  if (value.type === 'activity.ready') return { type: 'ready' };
  if (value.type === 'activity.error') {
    const message = boundedText(value.message, 500);
    return message ? { type: 'error', message } : null;
  }
  if (value.type !== 'context.propose' || !isRecord(value.payload)) return null;
  const title = boundedText(value.payload.title, 80);
  const summary = boundedText(value.payload.summary, 1_000);
  const prompt = boundedText(value.payload.prompt, 8_000);
  if (!title || !summary || !prompt) return null;
  if (value.payload.usePackageSkill !== undefined && typeof value.payload.usePackageSkill !== 'boolean') return null;
  const usePackageSkill = value.payload.usePackageSkill ?? true;
  const rawFields = Array.isArray(value.payload.fields) ? value.payload.fields.slice(0, 12) : [];
  const fields = rawFields.flatMap((field) => {
    if (!isRecord(field)) return [];
    const label = boundedText(field.label, 60);
    const fieldValue = boundedText(field.value, 500);
    return label && fieldValue ? [{ label, value: fieldValue }] : [];
  });
  return {
    type: 'context',
    proposal: { sourceKey, title, summary, prompt, fields, usePackageSkill },
  };
}

export function activityHostInit(theme: 'light' | 'dark', locale: string, packageId: string, key: string) {
  return {
    protocol: activityProtocol,
    type: 'host.init',
    payload: { theme, locale, packageId, key },
  } as const;
}

function boundedText(value: unknown, maxCharacters: number): string | null {
  if (typeof value !== 'string') return null;
  const normalized = value.trim();
  if (!normalized || normalized.length > maxCharacters) return null;
  return normalized;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
