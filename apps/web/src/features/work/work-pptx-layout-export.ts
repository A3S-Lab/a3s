import { withPresentationDesign } from './work-presentation-layouts';
import type {
  WorkPresentationContent,
  WorkPresentationLayout,
  WorkPresentationMaster,
  WorkSlideElement,
} from './work-types';

type PptxConstructor = typeof import('pptxgenjs').default;
type PptxPresentation = InstanceType<PptxConstructor>;
type PptxSlideMaster = Parameters<PptxPresentation['defineSlideMaster']>[0];
type PptxSlideMasterObject = NonNullable<PptxSlideMaster['objects']>[number];

export interface PptxLayoutBinding {
  masterName: string;
  placeholderNames: Map<string, string>;
}

export function definePptxSlideLayouts(
  presentation: PptxPresentation,
  source: WorkPresentationContent,
  slideWidth: number,
  slideHeight: number
): {
  content: WorkPresentationContent;
  bindings: Map<string, PptxLayoutBinding>;
} {
  const content = withPresentationDesign(source);
  const bindings = new Map<string, PptxLayoutBinding>();
  const usedNames = new Set<string>();
  for (const layout of content.layouts ?? []) {
    const master = content.masters?.find((candidate) => candidate.id === layout.masterId);
    const masterName = uniqueMasterName(layout.name, usedNames);
    const placeholders = effectivePlaceholders(master, layout);
    const placeholderNames = new Map(
      placeholders.map((element, index) => [
        element.placeholder?.key ?? `placeholder:${index + 1}`,
        placeholderName(element, index),
      ])
    );
    const inherited = [
      ...(layout.showMasterElements === false ? [] : (master?.elements ?? [])),
      ...layout.elements,
    ].filter((element) => !element.placeholder);
    const objects = [
      ...inherited.flatMap((element) => slideMasterObjects(element, slideWidth, slideHeight)),
      ...placeholders.map((element, index) =>
        slideMasterPlaceholder(
          element,
          placeholderNames.get(element.placeholder?.key ?? `placeholder:${index + 1}`) ??
            placeholderName(element, index),
          slideWidth,
          slideHeight
        )
      ),
    ];
    presentation.defineSlideMaster({
      title: masterName,
      background: {
        color: colorValue(layout.background ?? master?.background ?? '#ffffff'),
      },
      objects,
    });
    bindings.set(layout.id, { masterName, placeholderNames });
  }
  return { content, bindings };
}

function effectivePlaceholders(
  master: WorkPresentationMaster | undefined,
  layout: WorkPresentationLayout
): WorkSlideElement[] {
  const placeholders = new Map<string, WorkSlideElement>();
  for (const element of [
    ...(layout.showMasterElements === false ? [] : (master?.elements ?? [])),
    ...layout.elements,
  ]) {
    if (element.placeholder) placeholders.set(element.placeholder.key, element);
  }
  return Array.from(placeholders.values());
}

function slideMasterObjects(
  element: WorkSlideElement,
  slideWidth: number,
  slideHeight: number
): PptxSlideMasterObject[] {
  const box = elementBox(element, slideWidth, slideHeight);
  if (element.type === 'image' && element.image) {
    return [
      {
        image: {
          data: element.image.dataUrl,
          ...box,
          rotate: element.rotation,
          altText: element.altText,
        },
      },
    ];
  }
  if (element.type === 'line') {
    return [
      {
        line: {
          ...box,
          rotate: element.rotation,
          line: {
            color: colorValue(element.borderColor ?? element.color),
            width: element.borderWidth ?? 1,
          },
        },
      },
    ];
  }
  if (element.type === 'text') {
    return [
      {
        text: {
          text: element.text,
          options: textOptions(element, slideWidth, slideHeight),
        },
      },
    ];
  }
  if (element.type !== 'shape') return [];
  const objects: PptxSlideMasterObject[] = [
    {
      rect: {
        ...box,
        rotate: element.rotation,
        fill:
          element.fill === 'transparent'
            ? { color: 'FFFFFF', transparency: 100 }
            : {
                color: colorValue(element.fill),
                transparency: Math.round((1 - (element.opacity ?? 1)) * 100),
              },
        line: {
          color: colorValue(element.borderColor ?? element.fill),
          width: element.borderWidth ?? 0,
          transparency: element.borderWidth ? 0 : 100,
        },
      },
    },
  ];
  if (element.text) {
    objects.push({
      text: {
        text: element.text,
        options: textOptions(element, slideWidth, slideHeight),
      },
    });
  }
  return objects;
}

function slideMasterPlaceholder(
  element: WorkSlideElement,
  name: string,
  slideWidth: number,
  slideHeight: number
): PptxSlideMasterObject {
  return {
    placeholder: {
      text: element.placeholder?.prompt ?? element.text,
      options: {
        name,
        type: placeholderType(element.placeholder?.type),
        ...textOptions(element, slideWidth, slideHeight),
      },
    },
  };
}

function textOptions(element: WorkSlideElement, slideWidth: number, slideHeight: number) {
  return {
    ...elementBox(element, slideWidth, slideHeight),
    rotate: element.rotation,
    fontFace: element.fontFamily ?? 'Aptos',
    fontSize: Math.max(8, element.fontSize * 0.75),
    color: colorValue(element.color),
    bold: element.bold,
    italic: element.italic,
    underline: element.underline ? ({ style: 'sng' } as const) : undefined,
    align: element.align,
    valign: element.verticalAlign ?? ('middle' as const),
    margin: 0,
  };
}

function elementBox(element: WorkSlideElement, slideWidth: number, slideHeight: number) {
  return {
    x: (element.x / 100) * slideWidth,
    y: (element.y / 100) * slideHeight,
    w: (element.width / 100) * slideWidth,
    h: (element.height / 100) * slideHeight,
  };
}

function placeholderName(element: WorkSlideElement, index: number): string {
  const type = element.placeholder?.type || 'body';
  const key = element.placeholder?.key.replace(/[^a-z0-9_-]/gi, '-') || `${index + 1}`;
  return `A3S-${type}-${key}`;
}

function placeholderType(type: string | undefined): 'title' | 'body' | 'pic' | 'chart' | 'tbl' | 'media' {
  if (type === 'title' || type === 'ctrTitle') return 'title';
  if (type === 'pic') return 'pic';
  if (type === 'chart') return 'chart';
  if (type === 'tbl') return 'tbl';
  if (type === 'media') return 'media';
  return 'body';
}

function uniqueMasterName(name: string, used: Set<string>): string {
  const base = name.trim() || 'Slide layout';
  let candidate = base;
  let index = 2;
  while (used.has(candidate)) {
    candidate = `${base} ${index}`;
    index += 1;
  }
  used.add(candidate);
  return candidate;
}

function colorValue(color: string): string {
  return color.replace(/^#/, '').toUpperCase();
}
