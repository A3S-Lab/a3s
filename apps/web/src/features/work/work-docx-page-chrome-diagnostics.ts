import { docxFieldInstructions } from './work-docx-field-instructions';
import { descendants, type OoxmlPackage } from './work-ooxml-package';
import type { WorkCompatibilityIssue } from './work-types';

const PAGE_FIELD = /^\s*PAGE(?:\s|\\|$)/i;
const ADVANCED_POSITIONING = new Set([
  'anchor',
  'framePr',
  'ind',
  'pict',
  'positionH',
  'positionV',
  'simplePos',
  'tabs',
  'tblpPr',
  'txbxContent',
  'wrapNone',
  'wrapSquare',
  'wrapThrough',
  'wrapTight',
  'wrapTopAndBottom',
]);

export async function diagnoseDocxPageChrome(archive: OoxmlPackage): Promise<WorkCompatibilityIssue[]> {
  const parts = [...archive.paths('word/header'), ...archive.paths('word/footer')].filter((path) =>
    path.endsWith('.xml')
  );
  if (!parts.length) return [];

  const documents = await Promise.all(
    parts.map(async (path) => ({
      path,
      document: await archive.xml(path),
    }))
  );
  const issues: WorkCompatibilityIssue[] = [
    pageChromeIssue(
      'docx.headers',
      'Rich default, first-page, and even-page headers and footers, common formatting, tables, links, inline raster images, and PAGE numbering are preserved, editable, and used by preview, PDF, and native DOCX output.',
      'info'
    ),
  ];
  const instructions = documents.flatMap(({ document }) => docxFieldInstructions(document));
  if (instructions.some((instruction) => !PAGE_FIELD.test(instruction))) {
    issues.push(
      pageChromeIssue(
        'docx.headers.fields',
        'PAGE numbering is preserved. Other header/footer field codes are flattened to their current displayed text and no longer update automatically.'
      )
    );
  }
  if (
    documents.some(
      ({ path, document }) =>
        path.startsWith('word/header') &&
        docxFieldInstructions(document).some((instruction) => PAGE_FIELD.test(instruction))
    )
  ) {
    issues.push(
      pageChromeIssue(
        'docx.headers.page-field-position',
        "PAGE fields placed in a header are preserved through Work's centered footer page-number slot."
      )
    );
  }
  if (documents.some(({ document }) => descendants(document, 'sdt').length > 0)) {
    issues.push(
      pageChromeIssue(
        'docx.headers.content-controls',
        'Header/footer content controls are flattened to editable content; bindings, locking, and repeating behavior are not preserved.'
      )
    );
  }
  if (
    documents.some(({ document }) =>
      Array.from(document.querySelectorAll('*')).some((element) => ADVANCED_POSITIONING.has(element.localName))
    )
  ) {
    issues.push(
      pageChromeIssue(
        'docx.headers.positioning',
        'Floating drawings, text boxes, tab stops, indents, and advanced header/footer positioning are normalized to inline flow.'
      )
    );
  }
  return issues;
}

function pageChromeIssue(
  code: string,
  message: string,
  severity: WorkCompatibilityIssue['severity'] = 'warning'
): WorkCompatibilityIssue {
  return {
    code,
    feature: 'Headers and footers',
    message,
    severity,
  };
}
