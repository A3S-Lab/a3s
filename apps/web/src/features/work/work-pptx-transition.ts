import JSZip from 'jszip';
import { attribute, directChild, directChildren, parseXml } from './work-ooxml-package';
import { createWorkSlideTransition } from './work-presentation-transition';
import type {
  WorkSlide,
  WorkSlideTransition,
  WorkSlideTransitionDirection,
  WorkSlideTransitionSpeed,
} from './work-types';

export interface PptxTransitionDiagnostic {
  code: string;
  message: string;
}

export interface PptxTransitionReadResult {
  present: boolean;
  transition?: WorkSlideTransition;
  diagnostics: PptxTransitionDiagnostic[];
}

const EFFECT_TYPES = new Set(['fade', 'push', 'wipe', 'split', 'cut']);
const CARDINAL_DIRECTIONS: Record<string, WorkSlideTransitionDirection> = {
  l: 'left',
  r: 'right',
  u: 'up',
  d: 'down',
};
const OOXML_DIRECTIONS: Partial<Record<WorkSlideTransitionDirection, string>> = {
  left: 'l',
  right: 'r',
  up: 'u',
  down: 'd',
  in: 'in',
  out: 'out',
};

export function readPptxTransition(document: Document): PptxTransitionReadResult {
  const element = directChild(document.documentElement, 'transition');
  if (!element) return { present: false, diagnostics: [] };
  const diagnostics: PptxTransitionDiagnostic[] = [];
  if (directChild(element, 'sndAc')) {
    diagnostics.push({
      code: 'pptx.transition.sound',
      message: 'Transition sounds remain in the original PPTX only.',
    });
  }
  const effect = directChildren(element).find((child) => !['sndAc', 'extLst'].includes(child.localName));
  if (!effect || !EFFECT_TYPES.has(effect.localName)) {
    diagnostics.push({
      code: 'pptx.transition.type',
      message: effect
        ? `The “${effect.localName}” slide transition remains in the original PPTX only.`
        : 'The source slide has a transition container without a supported effect.',
    });
    return { present: true, diagnostics };
  }
  const base = createWorkSlideTransition(effect.localName as WorkSlideTransition['type']);
  const transition: WorkSlideTransition = {
    ...base,
    speed: readSpeed(element, diagnostics),
    advanceOnClick: readBoolean(attribute(element, 'advClick'), true),
  };
  const advanceAfterMs = nonnegativeInteger(attribute(element, 'advTm'));
  if (advanceAfterMs !== undefined) transition.advanceAfterMs = advanceAfterMs;
  else if (attribute(element, 'advTm') !== null) {
    diagnostics.push({
      code: 'pptx.transition.timing',
      message: 'An invalid automatic-advance delay was ignored.',
    });
  }
  if (transition.type === 'push' || transition.type === 'wipe') {
    transition.direction = CARDINAL_DIRECTIONS[attribute(effect, 'dir') ?? ''] ?? 'left';
  }
  if (transition.type === 'split') {
    transition.direction = attribute(effect, 'dir') === 'in' ? 'in' : 'out';
    transition.orientation = attribute(effect, 'orient') === 'vert' ? 'vertical' : 'horizontal';
  }
  if (readBoolean(attribute(effect, 'thruBlk'), false)) {
    diagnostics.push({
      code: 'pptx.transition.through-black',
      message: 'The through-black transition option is normalized to the basic effect.',
    });
  }
  return { present: true, transition, diagnostics };
}

export async function patchPptxTransitions(buffer: ArrayBuffer, slides: WorkSlide[]): Promise<ArrayBuffer> {
  if (!slides.some((slide) => slide.transition)) return buffer;
  const archive = await JSZip.loadAsync(buffer);
  for (const [index, slide] of slides.entries()) {
    const path = `ppt/slides/slide${index + 1}.xml`;
    const entry = archive.file(path);
    if (!entry) continue;
    const document = parseXml(await entry.async('text'), path);
    writePptxTransition(document, slide.transition);
    archive.file(path, new XMLSerializer().serializeToString(document));
  }
  return archive.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });
}

export function writePptxTransition(document: Document, transition: WorkSlideTransition | undefined): void {
  const root = document.documentElement;
  directChild(root, 'transition')?.remove();
  if (!transition) return;
  const namespace = root.namespaceURI;
  const prefix = root.lookupPrefix(namespace) ?? root.prefix ?? 'p';
  const element = document.createElementNS(namespace, `${prefix}:transition`);
  element.setAttribute('spd', transition.speed === 'medium' ? 'med' : transition.speed);
  element.setAttribute('advClick', transition.advanceOnClick ? '1' : '0');
  if (transition.advanceAfterMs !== undefined) {
    element.setAttribute('advTm', String(Math.max(0, Math.trunc(transition.advanceAfterMs))));
  }
  const effect = document.createElementNS(namespace, `${prefix}:${transition.type}`);
  if (transition.type === 'push' || transition.type === 'wipe') {
    effect.setAttribute('dir', OOXML_DIRECTIONS[transition.direction ?? 'left'] ?? 'l');
  }
  if (transition.type === 'split') {
    effect.setAttribute('orient', transition.orientation === 'vertical' ? 'vert' : 'horz');
    effect.setAttribute('dir', transition.direction === 'in' ? 'in' : 'out');
  }
  element.append(effect);
  const anchor = directChildren(root).find((child) => ['timing', 'extLst'].includes(child.localName)) ?? null;
  root.insertBefore(element, anchor);
}

function readSpeed(element: Element, diagnostics: PptxTransitionDiagnostic[]): WorkSlideTransitionSpeed {
  const value = attribute(element, 'spd');
  if (value === 'fast' || value === 'slow') return value;
  if (value === null || value === 'med') return 'medium';
  diagnostics.push({
    code: 'pptx.transition.speed',
    message: `The transition speed “${value}” was normalized to medium.`,
  });
  return 'medium';
}

function readBoolean(value: string | null, fallback: boolean): boolean {
  if (value === null) return fallback;
  return value === '1' || value.toLowerCase() === 'true';
}

function nonnegativeInteger(value: string | null): number | undefined {
  if (value === null) return undefined;
  const number = Number(value);
  return Number.isSafeInteger(number) && number >= 0 ? number : undefined;
}
