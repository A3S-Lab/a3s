import { describe, expect, it } from 'vitest';
import { activityProtocol, parsePluginMessage } from './plugin-protocol';

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
        'science:research'
      )
    ).toEqual({
      type: 'context',
      proposal: {
        sourceKey: 'science:research',
        title: 'Literature review',
        summary: 'Review recent CRISPR evidence.',
        prompt: 'Compare the selected sources.',
        fields: [{ label: 'Source', value: 'PubMed' }],
        usePackageSkill: false,
      },
    });
  });

  it('defaults legacy proposals to the verified package Skill and rejects invalid routing flags', () => {
    expect(
      parsePluginMessage(
        {
          protocol: activityProtocol,
          type: 'context.propose',
          payload: { title: 'Legacy', summary: 'Legacy proposal.', prompt: 'Continue.' },
        },
        'science:research'
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
        'science:research'
      )
    ).toBeNull();
  });

  it('rejects wrong protocols and oversized prompts', () => {
    expect(parsePluginMessage({ protocol: 'other', type: 'activity.ready' }, 'science:research')).toBeNull();
    expect(
      parsePluginMessage(
        {
          protocol: activityProtocol,
          type: 'context.propose',
          payload: { title: 'Title', summary: 'Summary', prompt: 'x'.repeat(8_001) },
        },
        'science:research'
      )
    ).toBeNull();
  });
});
