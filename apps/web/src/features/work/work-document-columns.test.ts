import { describe, expect, it } from 'vitest';
import {
  documentColumnGridTemplate,
  documentUnequalColumnGroups,
  normalizeDocumentColumns,
  parseDocumentColumns,
  serializeDocumentColumns,
} from './work-document-columns';

describe('Work document columns', () => {
  it('normalizes and serializes custom column proportions and gaps', () => {
    const columns = normalizeDocumentColumns({
      count: 2,
      spacing: 12,
      separator: true,
      custom: [
        { widthPercent: 65, spacing: 8 },
        { widthPercent: 35, spacing: 0 },
      ],
    });

    expect(columns).toEqual({
      count: 2,
      spacing: 12,
      separator: true,
      custom: [
        { widthPercent: 65, spacing: 8 },
        { widthPercent: 35, spacing: 0 },
      ],
    });
    expect(parseDocumentColumns(serializeDocumentColumns(columns))).toEqual(columns);
    expect(documentColumnGridTemplate(columns)).toBe('minmax(0, 65fr) 8mm minmax(0, 35fr)');
  });

  it('partitions top-level blocks into ordered proportional preview columns', () => {
    const columns = normalizeDocumentColumns({
      count: 2,
      spacing: 10,
      separator: false,
      custom: [
        { widthPercent: 60, spacing: 10 },
        { widthPercent: 40, spacing: 0 },
      ],
    });
    const groups = documentUnequalColumnGroups(
      '<p>Alpha paragraph</p><p>Beta paragraph</p><p>Gamma paragraph</p><p>Delta paragraph</p>',
      columns
    );

    expect(groups).toHaveLength(2);
    expect(groups.join('')).toMatch(/Alpha[\s\S]*Beta[\s\S]*Gamma[\s\S]*Delta/);
    expect(groups.every((group) => group.includes('<p>'))).toBe(true);
  });
});
