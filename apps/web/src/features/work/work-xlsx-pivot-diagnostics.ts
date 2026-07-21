import type { OoxmlPackage } from './work-ooxml-package';
import type { WorkCompatibilityIssue } from './work-types';
import { inspectXlsxPivotTables } from './work-xlsx-pivots';

export async function diagnoseXlsxPivots(archive: OoxmlPackage): Promise<WorkCompatibilityIssue[]> {
  const inspection = await inspectXlsxPivotTables(archive);
  const issues: WorkCompatibilityIssue[] = [];
  if (inspection.tables.length) {
    issues.push({
      code: 'xlsx.pivots',
      feature: 'Pivot tables',
      severity: 'info',
      message: `${inspection.tables.length} worksheet-backed pivot table(s) are editable, refreshable in Work, and regenerated with native XLSX definitions and caches.`,
    });
  }
  for (const unsupported of inspection.unsupported) {
    issues.push({
      code: unsupported.code,
      feature: 'Pivot tables',
      severity: 'warning',
      message: `${unsupported.message} Cached worksheet results remain visible, but a new Work export contains them as ordinary cells rather than an editable native pivot.`,
      location: unsupported.location,
    });
  }
  return issues;
}
