import { describe, expect, it } from 'vitest';
import type { PluginActivityDocumentIdentity } from './plugin-activity-document';
import { activityHostInit, activityProtocol, parsePluginMessage } from './plugin-protocol';

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
    expect(activityProtocol).toBe('a3s.activity.v2');
    expect(activityHostInit('dark', 'zh-CN', 'use/a3s/science', documentIdentity)).toEqual({
      protocol: 'a3s.activity.v2',
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
});
