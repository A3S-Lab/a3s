import type { WorkDocumentContent, WorkDocumentMargins } from './work-types';

export const DEFAULT_DOCUMENT_MARGINS: WorkDocumentMargins = {
  top: 25,
  right: 23,
  bottom: 25,
  left: 23,
};

export function documentMargins(content: WorkDocumentContent): WorkDocumentMargins {
  return {
    top: validMargin(content.margins?.top, DEFAULT_DOCUMENT_MARGINS.top),
    right: validMargin(content.margins?.right, DEFAULT_DOCUMENT_MARGINS.right),
    bottom: validMargin(content.margins?.bottom, DEFAULT_DOCUMENT_MARGINS.bottom),
    left: validMargin(content.margins?.left, DEFAULT_DOCUMENT_MARGINS.left),
  };
}

export function clampDocumentMargin(value: number): number {
  return Math.min(60, Math.max(5, Math.round(value * 10) / 10));
}

export function millimetersToPixels(value: number): number {
  return (value * 96) / 25.4;
}

function validMargin(value: number | undefined, fallback: number): number {
  return Number.isFinite(value) ? clampDocumentMargin(value as number) : fallback;
}
