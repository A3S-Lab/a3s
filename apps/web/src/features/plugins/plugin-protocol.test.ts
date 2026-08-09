import { describe, expect, it } from 'vitest';
import type { PluginActivityDocumentIdentity } from './plugin-activity-document';
import {
  activityHostInit,
  activityProtocol,
  activityStateError,
  activityStateResult,
  parsePluginMessage,
} from './plugin-protocol';

const documentIdentity: PluginActivityDocumentIdentity = {
  key: 'science:research',
  generation: 2,
  revision: 'b'.repeat(64),
  url: `/api/v1/plugins/activities/science%3Aresearch/document?generation=2&revision=${'b'.repeat(64)}`,
  token: `science:research:2:${'b'.repeat(64)}`,
};

describe('plugin activity protocol', () => {
  it('accepts bounded context proposals and binds them to the host source key', () => {
    expect(
      parsePluginMessage(
        {
          protocol: activityProtocol,
          type: 'context.propose',
          payload: {
            title: 'Literature review',
            summary: 'Review recent CRISPR evidence.',
            prompt: 'Compare the selected sources.',
            fields: [{ label: 'Source', value: 'PubMed' }],
            usePackageSkill: false,
            skill: 'untrusted-skill',
          },
        },
        documentIdentity
      )
    ).toEqual({
      type: 'context',
      proposal: {
        sourceKey: 'science:research',
        sourceGeneration: 2,
        sourceRevision: 'b'.repeat(64),
        sourceDocumentUrl: documentIdentity.url,
        title: 'Literature review',
        summary: 'Review recent CRISPR evidence.',
        prompt: 'Compare the selected sources.',
        fields: [{ label: 'Source', value: 'PubMed' }],
        usePackageSkill: false,
      },
    });
  });

  it('defaults an omitted routing choice to the verified package Skill and rejects invalid flags', () => {
    expect(
      parsePluginMessage(
        {
          protocol: activityProtocol,
          type: 'context.propose',
          payload: { title: 'Legacy', summary: 'Legacy proposal.', prompt: 'Continue.' },
        },
        documentIdentity
      )
    ).toMatchObject({ type: 'context', proposal: { usePackageSkill: true } });

    expect(
      parsePluginMessage(
        {
          protocol: activityProtocol,
          type: 'context.propose',
          payload: {
            title: 'Invalid',
            summary: 'Invalid routing flag.',
            prompt: 'Continue.',
            usePackageSkill: 'false',
          },
        },
        documentIdentity
      )
    ).toBeNull();
  });

  it('rejects wrong protocols and oversized prompts', () => {
    expect(parsePluginMessage({ protocol: 'other', type: 'activity.ready' }, documentIdentity)).toBeNull();
    expect(
      parsePluginMessage(
        {
          protocol: activityProtocol,
          type: 'context.propose',
          payload: { title: 'Title', summary: 'Summary', prompt: 'x'.repeat(8_001) },
        },
        documentIdentity
      )
    ).toBeNull();
  });

  it('binds host initialization to the exact document generation carried by the MessagePort', () => {
    expect(activityProtocol).toBe('a3s.activity.v3');
    expect(activityHostInit('dark', 'zh-CN', 'use/a3s/science', documentIdentity)).toEqual({
      protocol: 'a3s.activity.v3',
      type: 'host.init',
      payload: {
        theme: 'dark',
        locale: 'zh-CN',
        packageId: 'use/a3s/science',
        key: 'science:research',
        generation: 2,
        revision: 'b'.repeat(64),
      },
    });
  });

  it('accepts only bounded state requests with explicit correlation IDs', () => {
    expect(
      parsePluginMessage(
        { protocol: activityProtocol, type: 'state.get', requestId: 'request-1', key: 'filters.active' },
        documentIdentity
      )
    ).toEqual({ type: 'state', requestId: 'request-1', request: { operation: 'get', key: 'filters.active' } });
    expect(
      parsePluginMessage(
        {
          protocol: activityProtocol,
          type: 'state.set',
          requestId: 'request-2',
          key: 'draft/current',
          value: { query: 'CRISPR' },
        },
        documentIdentity
      )
    ).toEqual({
      type: 'state',
      requestId: 'request-2',
      request: { operation: 'set', key: 'draft/current', value: { query: 'CRISPR' } },
    });
    expect(
      parsePluginMessage({ protocol: activityProtocol, type: 'state.clear', requestId: 'request-3' }, documentIdentity)
    ).toEqual({ type: 'state', requestId: 'request-3', request: { operation: 'clear' } });

    expect(
      parsePluginMessage(
        { protocol: activityProtocol, type: 'state.get', requestId: '../request', key: 'valid' },
        documentIdentity
      )
    ).toBeNull();
    expect(
      parsePluginMessage(
        { protocol: activityProtocol, type: 'state.set', requestId: 'request-4', key: '../escape', value: null },
        documentIdentity
      )
    ).toBeNull();
    expect(
      parsePluginMessage(
        {
          protocol: activityProtocol,
          type: 'state.set',
          requestId: 'request-5',
          key: 'oversized',
          value: 'x'.repeat(16 * 1024),
        },
        documentIdentity
      )
    ).toBeNull();
  });

  it('builds correlated state results and errors', () => {
    expect(activityStateResult('request-1', { operation: 'get', found: true, value: 7 })).toEqual({
      protocol: activityProtocol,
      type: 'state.result',
      requestId: 'request-1',
      payload: { operation: 'get', found: true, value: 7 },
    });
    expect(activityStateError('request-2', 'unavailable', 'Registry is converging.')).toEqual({
      protocol: activityProtocol,
      type: 'state.error',
      requestId: 'request-2',
      code: 'unavailable',
      message: 'Registry is converging.',
    });
  });
});
