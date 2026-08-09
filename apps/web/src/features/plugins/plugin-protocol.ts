import type { PluginActivityDocumentIdentity } from './plugin-activity-document';
import type { PluginContextProposal } from './plugin-state';

export const activityProtocol = 'a3s.activity.v3';
const MAX_MESSAGE_BYTES = 32 * 1024;
const MAX_REQUEST_ID_BYTES = 64;
const MAX_STATE_KEY_BYTES = 128;
const MAX_STATE_VALUE_BYTES = 16 * 1024;

export type PluginStateRequest =
  | { operation: 'get'; key: string }
  | { operation: 'set'; key: string; value: unknown }
  | { operation: 'delete'; key: string }
  | { operation: 'clear' };

export type PluginHostMessage =
  | { type: 'ready' }
  | { type: 'context'; proposal: PluginContextProposal }
  | { type: 'error'; message: string }
  | { type: 'state'; requestId: string; request: PluginStateRequest };

export function parsePluginMessage(
  value: unknown,
  documentIdentity: PluginActivityDocumentIdentity
): PluginHostMessage | null {
  if (!isRecord(value)) return null;
  try {
    if (utf8Size(JSON.stringify(value)) > MAX_MESSAGE_BYTES) return null;
  } catch {
    return null;
  }
  if (value.protocol !== activityProtocol || typeof value.type !== 'string') return null;
  if (value.type === 'activity.ready') return { type: 'ready' };
  if (value.type === 'activity.error') {
    const message = boundedText(value.message, 500);
    return message ? { type: 'error', message } : null;
  }
  if (value.type.startsWith('state.')) return parseStateMessage(value);
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
    proposal: {
      sourceKey: documentIdentity.key,
      sourceGeneration: documentIdentity.generation,
      sourceRevision: documentIdentity.revision,
      sourceDocumentUrl: documentIdentity.url,
      title,
      summary,
      prompt,
      fields,
      usePackageSkill,
    },
  };
}

export function activityStateResult(requestId: string, payload: unknown) {
  return {
    protocol: activityProtocol,
    type: 'state.result',
    requestId,
    payload,
  } as const;
}

export function activityStateError(requestId: string, code: string, message: string) {
  return {
    protocol: activityProtocol,
    type: 'state.error',
    requestId,
    code,
    message,
  } as const;
}

export function activityHostInit(
  theme: 'light' | 'dark',
  locale: string,
  packageId: string,
  documentIdentity: PluginActivityDocumentIdentity
) {
  return {
    protocol: activityProtocol,
    type: 'host.init',
    payload: {
      theme,
      locale,
      packageId,
      key: documentIdentity.key,
      generation: documentIdentity.generation,
      revision: documentIdentity.revision,
    },
  } as const;
}

function boundedText(value: unknown, maxCharacters: number): string | null {
  if (typeof value !== 'string') return null;
  const normalized = value.trim();
  if (!normalized || normalized.length > maxCharacters) return null;
  return normalized;
}

function parseStateMessage(value: Record<string, unknown>): PluginHostMessage | null {
  const requestId = boundedMachineToken(value.requestId, MAX_REQUEST_ID_BYTES);
  if (!requestId) return null;
  if (value.type === 'state.clear') {
    return hasOnlyKeys(value, ['protocol', 'type', 'requestId'])
      ? { type: 'state', requestId, request: { operation: 'clear' } }
      : null;
  }
  const key = boundedStateKey(value.key);
  if (!key) return null;
  if (value.type === 'state.get' || value.type === 'state.delete') {
    if (!hasOnlyKeys(value, ['protocol', 'type', 'requestId', 'key'])) return null;
    return {
      type: 'state',
      requestId,
      request: { operation: value.type === 'state.get' ? 'get' : 'delete', key },
    };
  }
  if (value.type !== 'state.set' || !hasOnlyKeys(value, ['protocol', 'type', 'requestId', 'key', 'value'])) {
    return null;
  }
  if (!Object.hasOwn(value, 'value')) return null;
  let serialized: string | undefined;
  try {
    serialized = JSON.stringify(value.value);
  } catch {
    return null;
  }
  if (serialized === undefined || utf8Size(serialized) > MAX_STATE_VALUE_BYTES) return null;
  return { type: 'state', requestId, request: { operation: 'set', key, value: value.value } };
}

function boundedMachineToken(value: unknown, maxBytes: number): string | null {
  if (typeof value !== 'string' || !value || utf8Size(value) > maxBytes) return null;
  return /^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(value) ? value : null;
}

function boundedStateKey(value: unknown): string | null {
  if (typeof value !== 'string' || !value || utf8Size(value) > MAX_STATE_KEY_BYTES) return null;
  return /^[A-Za-z0-9][A-Za-z0-9._:/-]*$/.test(value) ? value : null;
}

function hasOnlyKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {
  const allowedKeys = new Set(allowed);
  return Object.keys(value).every((key) => allowedKeys.has(key));
}

function utf8Size(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
