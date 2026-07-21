import { createWorkId } from './work-templates';
import type {
  WorkPresentationContent,
  WorkPresentationLayout,
  WorkPresentationMaster,
  WorkSlide,
  WorkSlideElement,
} from './work-types';

export const DEFAULT_PRESENTATION_MASTER_ID = 'presentation-master-default';
export const DEFAULT_PRESENTATION_LAYOUT_ID = 'presentation-layout-default';

export interface WorkPresentationSlideView {
  background: string;
  inheritedElements: WorkSlideElement[];
  placeholderElements: WorkSlideElement[];
  master?: WorkPresentationMaster;
  layout?: WorkPresentationLayout;
}

export function withPresentationDesign(content: WorkPresentationContent): WorkPresentationContent {
  const masters = content.masters?.length
    ? content.masters
    : [
        {
          id: DEFAULT_PRESENTATION_MASTER_ID,
          name: 'Default master',
          background: '#ffffff',
          elements: [],
        },
      ];
  const masterIds = new Set(masters.map((master) => master.id));
  const firstMasterId = masters[0].id;
  const layouts = content.layouts?.length
    ? content.layouts.map((layout) => ({
        ...layout,
        masterId: masterIds.has(layout.masterId) ? layout.masterId : firstMasterId,
      }))
    : [
        {
          id: DEFAULT_PRESENTATION_LAYOUT_ID,
          name: 'Blank',
          masterId: firstMasterId,
          elements: [],
        },
      ];
  const layoutIds = new Set(layouts.map((layout) => layout.id));
  const firstLayoutId = layouts[0].id;
  return {
    ...content,
    masters,
    layouts,
    slides: content.slides.map((slide) => ({
      ...slide,
      layoutId: slide.layoutId && layoutIds.has(slide.layoutId) ? slide.layoutId : firstLayoutId,
      useLayoutBackground: slide.useLayoutBackground ?? false,
    })),
  };
}

export function presentationSlideView(content: WorkPresentationContent, slide: WorkSlide): WorkPresentationSlideView {
  const normalized = withPresentationDesign(content);
  const layout = normalized.layouts?.find((candidate) => candidate.id === slide.layoutId) ?? normalized.layouts?.[0];
  const master = normalized.masters?.find((candidate) => candidate.id === layout?.masterId) ?? normalized.masters?.[0];
  const showMasterElements = slide.showMasterElements !== false && layout?.showMasterElements !== false;
  const masterElements = showMasterElements ? (master?.elements ?? []) : [];
  const layoutElements = layout?.elements ?? [];
  const placeholderElements = mergedPlaceholders(masterElements, layoutElements);
  return {
    background:
      slide.useLayoutBackground === true
        ? (layout?.background ?? master?.background ?? slide.background)
        : slide.background,
    inheritedElements: [...masterElements, ...layoutElements].filter((element) => !element.placeholder),
    placeholderElements,
    master,
    layout,
  };
}

export function applyPresentationLayout(
  content: WorkPresentationContent,
  slideId: string,
  layoutId: string
): WorkPresentationContent {
  const normalized = withPresentationDesign(content);
  const layout = normalized.layouts?.find((candidate) => candidate.id === layoutId);
  if (!layout) return normalized;
  const definitions = layout.elements.filter(
    (element): element is WorkSlideElement & { placeholder: NonNullable<WorkSlideElement['placeholder']> } =>
      Boolean(element.placeholder)
  );
  return {
    ...normalized,
    slides: normalized.slides.map((slide) => {
      if (slide.id !== slideId) return slide;
      const matched = new Set<string>();
      const elements = slide.elements.map((element) => {
        const key = element.placeholder?.key;
        if (!key) return element;
        const definition = definitions.find((candidate) => candidate.placeholder.key === key);
        if (!definition) return { ...element, placeholder: undefined };
        matched.add(key);
        return applyPlaceholderDefinition(element, definition);
      });
      for (const definition of definitions) {
        if (matched.has(definition.placeholder.key)) continue;
        elements.push({
          ...structuredCopy(definition),
          id: createWorkId('element'),
          text: '',
          textRuns: undefined,
        });
      }
      return { ...slide, layoutId, elements };
    }),
  };
}

function mergedPlaceholders(
  masterElements: readonly WorkSlideElement[],
  layoutElements: readonly WorkSlideElement[]
): WorkSlideElement[] {
  const placeholders = new Map<string, WorkSlideElement>();
  for (const element of [...masterElements, ...layoutElements]) {
    if (element.placeholder) placeholders.set(element.placeholder.key, element);
  }
  return Array.from(placeholders.values());
}

function applyPlaceholderDefinition(element: WorkSlideElement, definition: WorkSlideElement): WorkSlideElement {
  return {
    ...structuredCopy(definition),
    id: element.id,
    type: element.type,
    text: element.text,
    textRuns: element.textRuns,
    image: element.image,
    table: element.table,
    chart: element.chart,
    href: element.href,
    altText: element.altText,
    placeholder: definition.placeholder ? { ...definition.placeholder } : element.placeholder,
  };
}

function structuredCopy<T>(value: T): T {
  if (typeof structuredClone === 'function') return structuredClone(value);
  return JSON.parse(JSON.stringify(value)) as T;
}
