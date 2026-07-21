import { describe, expect, it } from 'vitest';
import { createWorkArtifact, WORK_TEMPLATES } from './work-templates';

describe('Work templates', () => {
  it('offers complete document, spreadsheet, and presentation starting points', () => {
    expect(new Set(WORK_TEMPLATES.map((template) => template.kind))).toEqual(
      new Set(['document', 'spreadsheet', 'presentation'])
    );
    expect(WORK_TEMPLATES).toHaveLength(6);
  });

  it('creates editable spreadsheet and presentation content', () => {
    const spreadsheet = createWorkArtifact('quarterly-plan');
    const presentation = createWorkArtifact('strategy-deck');

    expect(spreadsheet.content.type).toBe('spreadsheet');
    if (spreadsheet.content.type === 'spreadsheet') {
      expect(spreadsheet.content.sheets[0]?.data?.[3]?.[5]?.f).toContain('SUM');
    }
    expect(presentation.content.type).toBe('presentation');
    if (presentation.content.type === 'presentation') {
      expect(presentation.content.slides).toHaveLength(3);
      expect(presentation.content.slides[0]?.elements.some((element) => element.text.includes('业务策略'))).toBe(true);
    }
  });
});
