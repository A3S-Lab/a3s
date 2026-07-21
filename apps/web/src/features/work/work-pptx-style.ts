import { attribute, directChildren, firstDescendant, OoxmlPackage, type OoxmlRelationship } from './work-ooxml-package';
import type { WorkSlideShapeType, WorkSlideTextAlign, WorkSlideVerticalAlign } from './work-types';

export type PptxThemeColors = Record<string, string>;

export interface PptxFillValue {
  color: string;
  opacity?: number;
}

export async function loadPptxTheme(
  archive: OoxmlPackage,
  presentationRelationships: Map<string, OoxmlRelationship>
): Promise<PptxThemeColors> {
  const defaults: PptxThemeColors = {
    dk1: '#000000',
    lt1: '#ffffff',
    dk2: '#1f497d',
    lt2: '#eeece1',
    accent1: '#4472c4',
    accent2: '#ed7d31',
    accent3: '#a5a5a5',
    accent4: '#ffc000',
    accent5: '#5b9bd5',
    accent6: '#70ad47',
    hlink: '#0563c1',
    folHlink: '#954f72',
    tx1: '#000000',
    tx2: '#1f497d',
    bg1: '#ffffff',
    bg2: '#eeece1',
  };
  const masterRelationship = Array.from(presentationRelationships.values()).find((item) =>
    item.type.endsWith('/slideMaster')
  );
  if (!masterRelationship || !archive.has(masterRelationship.target)) return defaults;
  const masterRelationships = await archive.relationships(masterRelationship.target);
  const themeRelationship = Array.from(masterRelationships.values()).find((item) => item.type.endsWith('/theme'));
  if (!themeRelationship || !archive.has(themeRelationship.target)) return defaults;
  const document = await archive.xml(themeRelationship.target);
  const colorScheme = firstDescendant(document, 'clrScheme');
  if (!colorScheme) return defaults;
  for (const slot of directChildren(colorScheme)) {
    const color = drawingColor(slot, defaults);
    if (color) defaults[slot.localName] = color.color;
  }
  return defaults;
}

export function readPptxBackground(document: Document, theme: PptxThemeColors): string | undefined {
  const background = firstDescendant(document, 'bg');
  if (!background) return undefined;
  return readPptxFill(background, theme, '#ffffff').color;
}

export function readPptxFill(
  container: ParentNode | null | undefined,
  theme: PptxThemeColors,
  fallback: string
): PptxFillValue {
  if (!container) return { color: fallback };
  if (firstDescendant(container, 'noFill')) return { color: 'transparent' };
  const solidFill = firstDescendant(container, 'solidFill');
  const color = drawingColor(solidFill ?? container, theme);
  return color ?? { color: fallback };
}

export function supportedPptxShapeType(value: string): WorkSlideShapeType | undefined {
  const shapes: Record<string, WorkSlideShapeType> = {
    rect: 'rect',
    roundRect: 'roundRect',
    ellipse: 'ellipse',
    triangle: 'triangle',
    rtTriangle: 'triangle',
    diamond: 'diamond',
    line: 'line',
  };
  return shapes[value];
}

export function pptxTextAlignment(value: string | null): WorkSlideTextAlign {
  if (value === 'ctr') return 'center';
  if (value === 'r') return 'right';
  return 'left';
}

export function pptxVerticalAlignment(value: string | null): WorkSlideVerticalAlign {
  if (value === 'ctr') return 'middle';
  if (value === 'b') return 'bottom';
  return 'top';
}

function drawingColor(container: ParentNode, theme: PptxThemeColors): PptxFillValue | undefined {
  const colorNames = ['srgbClr', 'scrgbClr', 'sysClr', 'schemeClr', 'prstClr'];
  const colorNode =
    directChildren(container).find((node) => colorNames.includes(node.localName)) ??
    colorNames.map((name) => firstDescendant(container, name)).find(Boolean);
  if (!colorNode) return undefined;
  let color: string | undefined;
  if (colorNode.localName === 'srgbClr') color = normalizeColor(attribute(colorNode, 'val'));
  else if (colorNode.localName === 'sysClr') color = normalizeColor(attribute(colorNode, 'lastClr'));
  else if (colorNode.localName === 'schemeClr') color = theme[attribute(colorNode, 'val') ?? ''];
  else if (colorNode.localName === 'prstClr') color = presetColor(attribute(colorNode, 'val'));
  else {
    const red = (numberAttribute(colorNode, 'r', 0) / 100_000) * 255;
    const green = (numberAttribute(colorNode, 'g', 0) / 100_000) * 255;
    const blue = (numberAttribute(colorNode, 'b', 0) / 100_000) * 255;
    color = rgbToHex(red, green, blue);
  }
  if (!color) return undefined;
  const tint = numberAttribute(firstDescendant(colorNode, 'tint'), 'val', 0) / 100_000;
  const shadeNode = firstDescendant(colorNode, 'shade');
  const shade = shadeNode ? numberAttribute(shadeNode, 'val', 100_000) / 100_000 : 1;
  color = modifyColor(color, tint, shade);
  const alphaNode = firstDescendant(colorNode, 'alpha');
  return {
    color,
    opacity: alphaNode ? numberAttribute(alphaNode, 'val', 100_000) / 100_000 : undefined,
  };
}

function normalizeColor(value: string | null): string | undefined {
  if (!value) return undefined;
  const normalized = value.replace(/^#/, '');
  return /^[0-9a-f]{6}$/i.test(normalized) ? `#${normalized.toLowerCase()}` : undefined;
}

function presetColor(value: string | null): string | undefined {
  const colors: Record<string, string> = {
    black: '#000000',
    white: '#ffffff',
    red: '#ff0000',
    green: '#008000',
    blue: '#0000ff',
    yellow: '#ffff00',
    orange: '#ffa500',
    purple: '#800080',
    gray: '#808080',
    grey: '#808080',
  };
  return value ? colors[value] : undefined;
}

function modifyColor(color: string, tint: number, shade: number): string {
  const red = Number.parseInt(color.slice(1, 3), 16);
  const green = Number.parseInt(color.slice(3, 5), 16);
  const blue = Number.parseInt(color.slice(5, 7), 16);
  return rgbToHex(
    (red + (255 - red) * tint) * shade,
    (green + (255 - green) * tint) * shade,
    (blue + (255 - blue) * tint) * shade
  );
}

function rgbToHex(red: number, green: number, blue: number): string {
  return `#${[red, green, blue]
    .map((value) =>
      Math.max(0, Math.min(255, Math.round(value)))
        .toString(16)
        .padStart(2, '0')
    )
    .join('')}`;
}

function numberAttribute(element: Element | undefined, name: string, fallback: number): number {
  if (!element) return fallback;
  const value = Number(attribute(element, name));
  return Number.isFinite(value) ? value : fallback;
}
