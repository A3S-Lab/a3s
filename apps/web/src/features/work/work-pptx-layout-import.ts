import {
  attribute,
  childPath,
  directChild,
  descendants,
  firstDescendant,
  OoxmlPackage,
  type OoxmlRelationship,
} from './work-ooxml-package';
import { type PptxThemeColors, readPptxBackground } from './work-pptx-style';
import type { WorkSlidePlaceholder } from './work-types';

export interface PptxRawBox {
  x: number;
  y: number;
  width: number;
  height: number;
  rotation?: number;
}

export interface PptxDesignPart {
  path: string;
  document: Document;
  relationships: Map<string, OoxmlRelationship>;
  name: string;
  background?: string;
  showMasterElements: boolean;
  sourceType?: string;
}

export interface PptxSlideInheritance {
  layout?: PptxDesignPart;
  master?: PptxDesignPart;
  placeholders: Map<string, PptxRawBox>;
}

export async function loadPptxSlideInheritance(
  archive: OoxmlPackage,
  slideRelationships: Map<string, OoxmlRelationship>,
  theme: PptxThemeColors
): Promise<PptxSlideInheritance> {
  const layoutRelationship = Array.from(slideRelationships.values()).find((item) => item.type.endsWith('/slideLayout'));
  const layout = layoutRelationship
    ? await loadPptxDesignPart(archive, layoutRelationship.target, theme, 'Slide layout')
    : undefined;
  const masterRelationship = layout
    ? Array.from(layout.relationships.values()).find((item) => item.type.endsWith('/slideMaster'))
    : undefined;
  const master = masterRelationship
    ? await loadPptxDesignPart(archive, masterRelationship.target, theme, 'Slide master')
    : undefined;
  const placeholders = new Map<string, PptxRawBox>();
  for (const part of [master, layout]) {
    if (!part) continue;
    for (const shape of descendants(part.document, 'sp')) {
      const box = pptxRawElementBox(shape);
      const placeholder = pptxPlaceholder(shape);
      if (!box || !placeholder) continue;
      placeholders.set(placeholder.key, box);
      placeholders.set(`type:${placeholder.type}`, box);
    }
  }
  return { layout, master, placeholders };
}

export function pptxRawElementBox(node: Element): PptxRawBox | undefined {
  const xfrm =
    node.localName === 'graphicFrame'
      ? directChild(node, 'xfrm')
      : node.localName === 'grpSp'
        ? childPath(node, 'grpSpPr', 'xfrm')
        : childPath(node, 'spPr', 'xfrm');
  if (!xfrm) return undefined;
  const offset = directChild(xfrm, 'off');
  const extent = directChild(xfrm, 'ext');
  if (!offset || !extent) return undefined;
  return {
    x: numberAttribute(offset, 'x', 0),
    y: numberAttribute(offset, 'y', 0),
    width: Math.abs(numberAttribute(extent, 'cx', 0)),
    height: Math.abs(numberAttribute(extent, 'cy', 0)),
    rotation: numberAttribute(xfrm, 'rot', 0) / 60_000 || undefined,
  };
}

export function pptxPlaceholderBox(node: Element, placeholders: Map<string, PptxRawBox>): PptxRawBox | undefined {
  const placeholder = pptxPlaceholder(node);
  if (!placeholder) return undefined;
  return placeholders.get(placeholder.key) ?? placeholders.get(`type:${placeholder.type}`);
}

export function pptxPlaceholder(node: Element): WorkSlidePlaceholder | undefined {
  const placeholder = firstDescendant(node, 'ph');
  if (!placeholder) return undefined;
  const index = attribute(placeholder, 'idx')?.trim();
  const type = attribute(placeholder, 'type')?.trim() || 'body';
  return {
    key: index ? `idx:${index}` : `type:${type}`,
    type,
  };
}

export function pptxShowsMasterElements(document: Document): boolean {
  const value = attribute(document.documentElement, 'showMasterSp');
  return value !== '0' && value !== 'false';
}

export async function loadPptxDesignPart(
  archive: OoxmlPackage,
  path: string,
  theme: PptxThemeColors,
  fallbackName: string
): Promise<PptxDesignPart | undefined> {
  if (!archive.has(path)) return undefined;
  const document = await archive.xml(path);
  const commonSlideData = directChild(document.documentElement, 'cSld');
  return {
    path,
    document,
    relationships: await archive.relationships(path),
    name:
      attribute(commonSlideData ?? document.documentElement, 'name')?.trim() || `${fallbackName} ${partNumber(path)}`,
    background: readPptxBackground(document, theme),
    showMasterElements: pptxShowsMasterElements(document),
    sourceType: attribute(document.documentElement, 'type')?.trim() || undefined,
  };
}

function partNumber(path: string): string {
  return /(\d+)(?:\.xml)?$/i.exec(path)?.[1] ?? '1';
}

function numberAttribute(element: Element | undefined, name: string, fallback: number): number {
  if (!element) return fallback;
  const value = Number(attribute(element, name));
  return Number.isFinite(value) ? value : fallback;
}
