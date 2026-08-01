import type { SkillCatalogItem } from '../../types/api';

const skillMentionPattern = /(?:^|[\s([{'"“（【])\$([\p{L}\p{N}_][\p{L}\p{N}._-]*)/gu;

export function skillNamesFromMentions(content: string, skills: readonly SkillCatalogItem[]): string[] {
  const available = new Set(skills.filter((skill) => skill.enabled).map((skill) => skill.name));
  const selected: string[] = [];
  for (const match of content.matchAll(skillMentionPattern)) {
    const name = match[1] ?? '';
    if (!name || !available.has(name) || selected.includes(name)) continue;
    selected.push(name);
  }
  return selected;
}
