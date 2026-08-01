import { describe, expect, it } from 'vitest';
import type { SkillCatalogItem } from '../../types/api';
import { skillNamesFromMentions } from './skill-mentions';

const skills: SkillCatalogItem[] = [
  { name: 'review', command: '$review', description: 'Review code', enabled: true, sources: [] },
  { name: 'a3s-office', command: '$a3s-office', description: 'Edit Office files', enabled: true, sources: [] },
  { name: 'disabled', command: '$disabled', description: 'Disabled', enabled: false, sources: [] },
];

describe('Skill dollar mentions', () => {
  it('resolves enabled Skill mentions and ignores punctuation and duplicates', () => {
    expect(skillNamesFromMentions('$review 请使用（$a3s-office），再用 $review。', skills)).toEqual([
      'review',
      'a3s-office',
    ]);
  });

  it('leaves unknown, disabled, and currency tokens alone', () => {
    expect(skillNamesFromMentions('$unknown $disabled $5', skills)).toEqual([]);
  });
});
