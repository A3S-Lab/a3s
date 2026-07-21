import { attribute, descendants, directChild, directChildren } from './work-ooxml-package';
import type { WorkCompatibilityIssue } from './work-types';
import { inspectXlsxHeaderFooterText } from './work-xlsx-header-footer';
import { isEditableXlsxPaperSizeCode } from './work-xlsx-page-setup';

export function diagnoseXlsxPageSetup(document: Document): WorkCompatibilityIssue[] {
  const issues: WorkCompatibilityIssue[] = [];
  const root = document.documentElement;
  const pageSetup = directChild(root, 'pageSetup');
  const headerFooter = directChild(root, 'headerFooter');
  const hasPageSetup =
    Boolean(pageSetup) ||
    Boolean(directChild(root, 'pageMargins')) ||
    Boolean(directChild(root, 'printOptions')) ||
    descendants(root, 'pageSetUpPr').length > 0;
  if (hasPageSetup) {
    issues.push(
      issue(
        'xlsx.page-setup',
        'Page setup',
        'Paper size, orientation, percentage or fit-to-page scaling, margins, centering, first-page numbering, and page order are preserved, editable, and honored by PDF pagination.',
        'info'
      )
    );
  }
  if (pageSetup) inspectPrinterSetup(pageSetup, issues);
  if (headerFooter) inspectHeaderFooter(headerFooter, issues);
  return issues;
}

function inspectPrinterSetup(pageSetup: Element, issues: WorkCompatibilityIssue[]): void {
  const paperSize = attribute(pageSetup, 'paperSize');
  if (paperSize && !isEditableXlsxPaperSizeCode(paperSize)) {
    issues.push(
      issue(
        'xlsx.page-setup.paper-size',
        'Page setup',
        `Paper-size code ${paperSize} is not editable yet and falls back to A4 in Work PDF output.`
      )
    );
  }
  const orientation = attribute(pageSetup, 'orientation')?.toLowerCase();
  if (orientation && orientation !== 'portrait' && orientation !== 'landscape') {
    issues.push(
      issue(
        'xlsx.page-setup.orientation',
        'Page setup',
        `Page orientation "${orientation}" uses a printer-specific default and falls back to landscape in Work PDF output.`
      )
    );
  }
  const unsupportedAttributes = [
    'blackAndWhite',
    'cellComments',
    'copies',
    'draft',
    'errors',
    'horizontalDpi',
    'paperHeight',
    'paperWidth',
    'usePrinterDefaults',
    'verticalDpi',
    'r:id',
  ].filter((name) => attribute(pageSetup, name) !== null);
  if (unsupportedAttributes.length) {
    issues.push(
      issue(
        'xlsx.page-setup.printer-settings',
        'Page setup',
        `Printer-specific page-setup controls remain in the original workbook only: ${unsupportedAttributes.join(', ')}.`
      )
    );
  }
}

function inspectHeaderFooter(headerFooter: Element, issues: WorkCompatibilityIssue[]): void {
  issues.push(
    issue(
      'xlsx.header-footer',
      'Print headers and footers',
      'Odd-page plain-text header and footer sections, dynamic fields, page-number start, scaling, and margin alignment are preserved, editable, and rendered in PDF output.',
      'info'
    )
  );
  const firstOrEvenParts = ['firstHeader', 'firstFooter', 'evenHeader', 'evenFooter'];
  const hasVariants =
    booleanAttribute(headerFooter, 'differentFirst') ||
    booleanAttribute(headerFooter, 'differentOddEven') ||
    firstOrEvenParts.some((name) => Boolean(directChild(headerFooter, name)));
  if (hasVariants) {
    issues.push(
      issue(
        'xlsx.header-footer.variants',
        'Print headers and footers',
        'First-page and even-page header/footer variants remain in the original workbook only; Work edits the odd-page template.'
      )
    );
  }
  const inspection = directChildren(headerFooter).reduce(
    (combined, element) => {
      const current = inspectXlsxHeaderFooterText(element.textContent);
      return {
        hasFormatting: combined.hasFormatting || current.hasFormatting,
        hasImage: combined.hasImage || current.hasImage,
      };
    },
    { hasFormatting: false, hasImage: false }
  );
  if (inspection.hasFormatting) {
    issues.push(
      issue(
        'xlsx.header-footer.formatting',
        'Print headers and footers',
        'Rich header/footer font formatting is normalized to editable plain text.'
      )
    );
  }
  if (inspection.hasImage) {
    issues.push(
      issue(
        'xlsx.header-footer.images',
        'Print headers and footers',
        'Header and footer images remain in the original workbook only.'
      )
    );
  }
}

function booleanAttribute(element: Element, name: string): boolean {
  const value = attribute(element, name)?.toLowerCase();
  return value === '1' || value === 'true';
}

function issue(
  code: string,
  feature: string,
  message: string,
  severity: WorkCompatibilityIssue['severity'] = 'warning'
): WorkCompatibilityIssue {
  return { code, feature, message, severity };
}
