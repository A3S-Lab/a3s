import {
  attribute,
  bytesToDataUrl,
  childPath,
  contentTypeForPart,
  descendants,
  directChild,
  directChildren,
  firstDescendant,
  OoxmlPackage,
  type OoxmlRelationship,
} from './work-ooxml-package';
import {
  loadPptxDesignPart,
  loadPptxSlideInheritance,
  type PptxDesignPart,
  pptxPlaceholder,
  pptxPlaceholderBox,
  type PptxRawBox,
  pptxRawElementBox,
  pptxShowsMasterElements,
} from './work-pptx-layout-import';
import {
  loadPptxTheme,
  type PptxThemeColors,
  pptxTextAlignment,
  pptxVerticalAlignment,
  readPptxBackground,
  readPptxFill,
  supportedPptxShapeType,
} from './work-pptx-style';
import { loadPptxCommentAuthors, type PptxCommentAuthor, readPptxSlideComments } from './work-pptx-comments';
import { readPptxChart } from './work-pptx-chart-import';
import { readPptxTransition } from './work-pptx-transition';
import { createWorkId } from './work-templates';
import type {
  WorkCompatibilityIssue,
  WorkCompatibilityReport,
  WorkPresentationContent,
  WorkPresentationLayout,
  WorkPresentationMaster,
  WorkSlide,
  WorkSlideElement,
  WorkSlideTextAlign,
  WorkSlideTextRun,
  WorkSlideVerticalAlign,
} from './work-types';

const EMU_PER_INCH = 914_400;
const DEFAULT_SLIDE_WIDTH = 13.333;
const DEFAULT_SLIDE_HEIGHT = 7.5;
const MAX_EMBEDDED_IMAGE_BYTES = 11 * 1024 * 1024;

interface PptxImportContext {
  archive: OoxmlPackage;
  slideNumber: number;
  slideWidthEmu: number;
  slideHeightEmu: number;
  relationships: Map<string, OoxmlRelationship>;
  placeholders: Map<string, PptxRawBox>;
  theme: PptxThemeColors;
  issues: WorkCompatibilityIssue[];
  imageBudget: { bytes: number };
  location?: string;
}

interface PptxDesignRegistry {
  masterByPath: Map<string, WorkPresentationMaster>;
  layoutByPath: Map<string, WorkPresentationLayout>;
  masters: WorkPresentationMaster[];
  layouts: WorkPresentationLayout[];
}

interface GroupTransform {
  map: (box: PptxRawBox) => PptxRawBox;
}

interface ParsedText {
  text: string;
  runs: WorkSlideTextRun[];
  align: WorkSlideTextAlign;
  verticalAlign: WorkSlideVerticalAlign;
  fontSize: number;
  color: string;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  fontFamily?: string;
  href?: string;
  explicitStyle: boolean;
}

export interface PptxImportResult {
  content: WorkPresentationContent;
  compatibility: WorkCompatibilityReport;
}

export async function importPptxPresentation(file: File): Promise<PptxImportResult> {
  const archive = await OoxmlPackage.load(await file.arrayBuffer());
  if (!archive.has('ppt/presentation.xml')) throw new Error('The selected file is not a valid PPTX presentation.');

  const presentation = await archive.xml('ppt/presentation.xml');
  const presentationRelationships = await archive.relationships('ppt/presentation.xml');
  const size = firstDescendant(presentation, 'sldSz');
  const slideWidthEmu = positiveNumber(attribute(size ?? presentation.documentElement, 'cx')) ?? 12_192_000;
  const slideHeightEmu = positiveNumber(attribute(size ?? presentation.documentElement, 'cy')) ?? 6_858_000;
  const theme = await loadPptxTheme(archive, presentationRelationships);
  const commentAuthors = await loadPptxCommentAuthors(archive, presentationRelationships);
  const issues: WorkCompatibilityIssue[] = [];
  const imageBudget = { bytes: 0 };
  const designs: PptxDesignRegistry = {
    masterByPath: new Map(),
    layoutByPath: new Map(),
    masters: [],
    layouts: [],
  };
  await loadPptxPresentationDesigns(
    archive,
    presentationRelationships,
    slideWidthEmu,
    slideHeightEmu,
    theme,
    issues,
    imageBudget,
    designs
  );
  const slideParts = orderedSlideParts(presentation, presentationRelationships);

  if (!slideParts.length) throw new Error('The PPTX presentation does not contain any slides.');

  const slides: WorkSlide[] = [];
  for (const [index, slidePart] of slideParts.entries()) {
    slides.push(
      await parseSlide(
        archive,
        slidePart,
        index + 1,
        slideWidthEmu,
        slideHeightEmu,
        theme,
        commentAuthors,
        issues,
        imageBudget,
        designs
      )
    );
  }
  if (designs.layouts.length || designs.masters.length) {
    issues.push({
      code: 'pptx.layouts',
      severity: 'info',
      feature: 'Slide layouts and masters',
      message: `${designs.layouts.length} slide layout(s) and ${designs.masters.length} slide master(s) retain inherited backgrounds, artwork, placeholder geometry, and native layout assignments.`,
    });
  }
  if (designs.masters.length > 1) {
    issues.push({
      code: 'pptx.masters.multiple',
      severity: 'warning',
      feature: 'Slide layouts and masters',
      message: `${designs.masters.length} source master trees remain separately editable in Work. PPTX export preserves their visual inheritance per layout while normalizing them into the generated master hierarchy.`,
    });
  }
  addFontDiagnostics(slides, issues);

  return {
    content: {
      type: 'presentation',
      slides,
      width: slideWidthEmu / EMU_PER_INCH || DEFAULT_SLIDE_WIDTH,
      height: slideHeightEmu / EMU_PER_INCH || DEFAULT_SLIDE_HEIGHT,
      masters: designs.masters.length ? designs.masters : undefined,
      layouts: designs.layouts.length ? designs.layouts : undefined,
    },
    compatibility: {
      sourceFormat: 'PPTX',
      sourceName: file.name,
      assessedAt: Date.now(),
      issues: deduplicateIssues(issues),
    },
  };
}

async function parseSlide(
  archive: OoxmlPackage,
  slidePart: string,
  slideNumber: number,
  slideWidthEmu: number,
  slideHeightEmu: number,
  theme: PptxThemeColors,
  commentAuthors: Map<string, PptxCommentAuthor>,
  issues: WorkCompatibilityIssue[],
  imageBudget: { bytes: number },
  designs: PptxDesignRegistry
): Promise<WorkSlide> {
  const document = await archive.xml(slidePart);
  const relationships = await archive.relationships(slidePart);
  const inheritance = await loadPptxSlideInheritance(archive, relationships, theme);
  const context: PptxImportContext = {
    archive,
    slideNumber,
    slideWidthEmu,
    slideHeightEmu,
    relationships,
    placeholders: inheritance.placeholders,
    theme,
    issues,
    imageBudget,
    location: `幻灯片 ${slideNumber}`,
  };
  const design = await ensurePptxDesignDefinitions(archive, inheritance.layout, inheritance.master, context, designs);
  const elements: WorkSlideElement[] = [];
  const shapeTree = firstDescendant(document, 'spTree');
  for (const child of shapeTree ? directChildren(shapeTree) : []) {
    elements.push(...(await parseSlideNode(child, context)));
  }
  const resolvedElements = inheritPptxPlaceholderStyles(elements, design.master, design.layout);

  const transition = readPptxTransition(document);
  const commentResult = await readPptxSlideComments(
    archive,
    relationships,
    commentAuthors,
    slideNumber,
    slideWidthEmu,
    slideHeightEmu
  );
  if (commentResult.comments.length) {
    addIssue(
      context,
      'pptx.comments',
      'Comments',
      `${commentResult.comments.length} traditional slide comment(s), authors, dates, and positions are preserved and editable.`,
      'info'
    );
  }
  if (commentResult.hasUnsupportedThreadedComments) {
    addIssue(
      context,
      'pptx.comments.threaded',
      'Modern comments',
      'Modern threaded replies, mentions, and resolved state remain in the original PPTX only.'
    );
  }
  if (commentResult.hasUnreadableComments) {
    addIssue(
      context,
      'pptx.comments.unreadable',
      'Comments',
      'One or more slide comment parts could not be read and remain in the original PPTX only.'
    );
  }
  if (commentResult.hasMalformedMetadata) {
    addIssue(
      context,
      'pptx.comments.metadata',
      'Comment metadata',
      'One or more traditional comments had a missing author, date, or position; unavailable metadata was normalized.'
    );
  }
  if (transition.present) {
    addIssue(
      context,
      'pptx.transition',
      'Slide transitions',
      transition.transition
        ? 'Basic slide transitions and advance timing are preserved, editable, and replayed in presentation preview.'
        : 'This slide transition is not editable and remains in the original PPTX only.',
      transition.transition ? 'info' : 'warning'
    );
    for (const diagnostic of transition.diagnostics) {
      addIssue(context, diagnostic.code, 'Slide transitions', diagnostic.message);
    }
  }
  if (firstDescendant(document, 'timing')) {
    addIssue(
      context,
      'pptx.animation',
      'Animations',
      'Object animations are not replayed and will be omitted on export.'
    );
  }
  for (const relationship of relationships.values()) {
    if (
      relationship.type.includes('/audio') ||
      relationship.type.includes('/video') ||
      relationship.type.includes('/media')
    ) {
      addIssue(
        context,
        'pptx.media',
        'Embedded media',
        'Audio and video remain in the original file but are not playable yet.'
      );
    } else if (relationship.type.endsWith('/oleObject') || relationship.type.endsWith('/package')) {
      addIssue(
        context,
        'pptx.ole',
        'Embedded objects',
        'Embedded Office or OLE objects remain in the original file only.'
      );
    }
  }

  const slideBackground = readPptxBackground(document, theme);
  return {
    id: createWorkId('slide'),
    name: readSlideName(document, slideNumber),
    layoutId: design.layout?.id,
    background: slideBackground ?? inheritance.layout?.background ?? inheritance.master?.background ?? '#ffffff',
    useLayoutBackground: !slideBackground,
    showMasterElements: pptxShowsMasterElements(document) ? undefined : false,
    elements: resolvedElements,
    notes: await readSpeakerNotes(archive, relationships, context),
    comments: commentResult.comments.length ? commentResult.comments : undefined,
    transition: transition.transition,
  };
}

async function loadPptxPresentationDesigns(
  archive: OoxmlPackage,
  relationships: Map<string, OoxmlRelationship>,
  slideWidthEmu: number,
  slideHeightEmu: number,
  theme: PptxThemeColors,
  issues: WorkCompatibilityIssue[],
  imageBudget: { bytes: number },
  registry: PptxDesignRegistry
): Promise<void> {
  const context: PptxImportContext = {
    archive,
    slideNumber: 0,
    slideWidthEmu,
    slideHeightEmu,
    relationships,
    placeholders: new Map(),
    theme,
    issues,
    imageBudget,
    location: '母版与布局',
  };
  for (const relationship of relationships.values()) {
    if (!relationship.type.endsWith('/slideMaster')) continue;
    const part = await loadPptxDesignPart(archive, relationship.target, theme, 'Slide master');
    if (part) await ensurePptxMaster(archive, part, context, registry);
  }
}

async function ensurePptxDesignDefinitions(
  archive: OoxmlPackage,
  layoutPart: PptxDesignPart | undefined,
  masterPart: PptxDesignPart | undefined,
  context: PptxImportContext,
  registry: PptxDesignRegistry
): Promise<{ master?: WorkPresentationMaster; layout?: WorkPresentationLayout }> {
  const master = masterPart
    ? await ensurePptxMaster(archive, masterPart, context, registry)
    : layoutPart
      ? ensureFallbackPptxMaster(registry)
      : undefined;
  const layout = layoutPart && master ? await ensurePptxLayout(layoutPart, master.id, context, registry) : undefined;
  return { master, layout };
}

async function ensurePptxMaster(
  archive: OoxmlPackage,
  part: PptxDesignPart,
  context: PptxImportContext,
  registry: PptxDesignRegistry
): Promise<WorkPresentationMaster> {
  const existing = registry.masterByPath.get(part.path);
  if (existing) return existing;
  const master: WorkPresentationMaster = {
    id: `pptx-master-${registry.masters.length + 1}`,
    name: part.name,
    background: part.background ?? '#ffffff',
    elements: await parsePptxDesignElements(part, context),
  };
  registry.masterByPath.set(part.path, master);
  registry.masters.push(master);
  for (const relationship of part.relationships.values()) {
    if (!relationship.type.endsWith('/slideLayout')) continue;
    const layoutPart = await loadPptxDesignPart(archive, relationship.target, context.theme, 'Slide layout');
    if (layoutPart) await ensurePptxLayout(layoutPart, master.id, context, registry);
  }
  return master;
}

async function ensurePptxLayout(
  part: PptxDesignPart,
  masterId: string,
  context: PptxImportContext,
  registry: PptxDesignRegistry
): Promise<WorkPresentationLayout> {
  const existing = registry.layoutByPath.get(part.path);
  if (existing) return existing;
  const layout: WorkPresentationLayout = {
    id: `pptx-layout-${registry.layouts.length + 1}`,
    name: part.name,
    masterId,
    background: part.background,
    elements: await parsePptxDesignElements(part, context),
    showMasterElements: part.showMasterElements ? undefined : false,
    sourceType: part.sourceType,
  };
  registry.layoutByPath.set(part.path, layout);
  registry.layouts.push(layout);
  return layout;
}

function ensureFallbackPptxMaster(registry: PptxDesignRegistry): WorkPresentationMaster {
  const path = 'pptx:fallback-master';
  const existing = registry.masterByPath.get(path);
  if (existing) return existing;
  const master: WorkPresentationMaster = {
    id: `pptx-master-${registry.masters.length + 1}`,
    name: 'Imported master',
    background: '#ffffff',
    elements: [],
  };
  registry.masterByPath.set(path, master);
  registry.masters.push(master);
  return master;
}

async function parsePptxDesignElements(part: PptxDesignPart, context: PptxImportContext): Promise<WorkSlideElement[]> {
  const partContext: PptxImportContext = {
    ...context,
    relationships: part.relationships,
    placeholders: new Map(),
  };
  const elements: WorkSlideElement[] = [];
  const shapeTree = firstDescendant(part.document, 'spTree');
  for (const child of shapeTree ? directChildren(shapeTree) : []) {
    elements.push(...(await parseSlideNode(child, partContext)));
  }
  return elements;
}

function inheritPptxPlaceholderStyles(
  elements: readonly WorkSlideElement[],
  master: WorkPresentationMaster | undefined,
  layout: WorkPresentationLayout | undefined
): WorkSlideElement[] {
  const definitions = new Map<string, WorkSlideElement>();
  const addDefinitions = (candidates: readonly WorkSlideElement[]) => {
    for (const candidate of candidates) {
      const placeholder = candidate.placeholder;
      if (!placeholder) continue;
      const inheritedDefinition = placeholder.inheritsStyle
        ? (definitions.get(placeholder.key) ?? definitions.get(`type:${placeholder.type}`))
        : undefined;
      const definition = inheritedDefinition ? inheritPptxPlaceholderStyle(candidate, inheritedDefinition) : candidate;
      definitions.set(placeholder.key, definition);
      definitions.set(`type:${placeholder.type}`, definition);
    }
  };

  if (layout?.showMasterElements !== false) addDefinitions(master?.elements ?? []);
  addDefinitions(layout?.elements ?? []);

  return elements.map((element) => {
    const placeholder = element.placeholder;
    if (!placeholder?.inheritsStyle) return element;
    const definition = definitions.get(placeholder.key) ?? definitions.get(`type:${placeholder.type}`);
    return definition ? inheritPptxPlaceholderStyle(element, definition) : element;
  });
}

function inheritPptxPlaceholderStyle(element: WorkSlideElement, definition: WorkSlideElement): WorkSlideElement {
  return {
    ...element,
    fontSize: definition.fontSize,
    color: definition.color,
    fill: definition.fill,
    bold: definition.bold,
    align: definition.align,
    fontFamily: definition.fontFamily,
    italic: definition.italic,
    underline: definition.underline,
    verticalAlign: definition.verticalAlign,
    textRuns: element.textRuns?.map((run) => ({
      ...run,
      fontSize: run.fontSize ?? definition.fontSize,
      color: run.color ?? definition.color,
      bold: run.bold ?? definition.bold,
      italic: run.italic ?? definition.italic,
      underline: run.underline ?? definition.underline,
      fontFamily: run.fontFamily ?? definition.fontFamily,
    })),
  };
}

async function parseSlideNode(
  node: Element,
  context: PptxImportContext,
  transform?: GroupTransform
): Promise<WorkSlideElement[]> {
  if (node.localName === 'sp') return [parseShape(node, context, transform)];
  if (node.localName === 'pic') return [await parsePicture(node, context, transform)];
  if (node.localName === 'graphicFrame') return [await parseGraphicFrame(node, context, transform)];
  if (node.localName === 'cxnSp') return [parseConnector(node, context, transform)];
  if (node.localName === 'grpSp') return parseGroup(node, context, transform);
  if (['contentPart', 'oleObj'].includes(node.localName)) {
    addIssue(
      context,
      'pptx.content-part',
      'Embedded content',
      'An embedded slide object remains available in the original PPTX only.'
    );
  }
  return [];
}

function parseShape(node: Element, context: PptxImportContext, transform?: GroupTransform): WorkSlideElement {
  const box = elementBox(node, context, transform);
  const text = parseText(firstDescendant(node, 'txBody'), context);
  const placeholder = pptxPlaceholder(node);
  const ownBox = pptxRawElementBox(node);
  const properties = directChild(node, 'spPr');
  const fill = readPptxFill(properties, context.theme, text.text ? 'transparent' : '#dce6fb');
  const geometry = attribute(firstDescendant(properties, 'prstGeom') ?? node, 'prst') ?? 'rect';
  const shapeType = supportedPptxShapeType(geometry);
  if (!shapeType) {
    addIssue(
      context,
      `pptx.shape.${geometry}`,
      'Shape geometry',
      `The “${geometry}” shape is shown as a rectangle and will be normalized on export.`
    );
  }
  const line = firstDescendant(properties, 'ln');
  const border = readPptxFill(line, context.theme, fill.color === 'transparent' ? '#657087' : fill.color);
  const isTextBox = attribute(firstDescendant(node, 'cNvSpPr') ?? node, 'txBox') === '1';
  const isTextPlaceholder = placeholder && !['pic', 'chart', 'tbl', 'media', 'obj'].includes(placeholder.type);
  const altText = readAlternativeText(node);
  return {
    id: createWorkId('element'),
    type:
      shapeType === 'line'
        ? 'line'
        : isTextPlaceholder || isTextBox || (text.text && fill.color === 'transparent')
          ? 'text'
          : 'shape',
    ...box,
    text: text.text,
    fontSize: text.fontSize,
    color: text.color,
    fill: fill.color,
    bold: text.bold,
    align: text.align,
    shapeType: shapeType ?? 'rect',
    radius: shapeType === 'roundRect' ? 5 : undefined,
    opacity: fill.opacity,
    borderColor: border.color,
    borderWidth: numberAttribute(line, 'w', 0) / 12_700 || undefined,
    fontFamily: text.fontFamily,
    italic: text.italic,
    underline: text.underline,
    verticalAlign: text.verticalAlign,
    textRuns: text.runs.length ? text.runs : undefined,
    href: text.href,
    altText,
    placeholder: placeholder
      ? {
          ...placeholder,
          prompt: text.text.trim() || undefined,
          inheritsGeometry: ownBox ? undefined : true,
          inheritsStyle: text.explicitStyle ? undefined : true,
        }
      : undefined,
  };
}

async function parsePicture(
  node: Element,
  context: PptxImportContext,
  transform?: GroupTransform
): Promise<WorkSlideElement> {
  const box = elementBox(node, context, transform);
  const properties = firstDescendant(node, 'blip');
  const relationshipId = attribute(properties ?? node, 'r:embed') ?? attribute(properties ?? node, 'embed');
  const relationship = relationshipId ? context.relationships.get(relationshipId) : undefined;
  const name = attribute(firstDescendant(node, 'cNvPr') ?? node, 'name') ?? 'Imported picture';
  const placeholder = pptxPlaceholder(node);
  const base = {
    id: createWorkId('element'),
    type: 'image' as const,
    ...box,
    text: '',
    fontSize: 12,
    color: '#536078',
    fill: '#eef1f6',
    bold: false,
    align: 'center' as const,
    altText: readAlternativeText(node) || name,
    placeholder,
  };
  if (!relationship || relationship.targetMode === 'External' || !context.archive.has(relationship.target)) {
    addIssue(context, 'pptx.image.linked', 'Linked pictures', `The linked picture “${name}” could not be embedded.`);
    return { ...base, type: 'shape', text: `图片：${name}` };
  }
  const bytes = await context.archive.bytes(relationship.target);
  if (context.imageBudget.bytes + bytes.byteLength > MAX_EMBEDDED_IMAGE_BYTES) {
    addIssue(
      context,
      'pptx.image.limit',
      'Large pictures',
      `The picture “${name}” exceeds the safe native-artifact image budget and remains in the original file only.`
    );
    return { ...base, type: 'shape', text: `大型图片：${name}` };
  }
  context.imageBudget.bytes += bytes.byteLength;
  const contentType = contentTypeForPart(relationship.target);
  const sourceRectangle = firstDescendant(node, 'srcRect');
  if (sourceRectangle && Array.from(sourceRectangle.attributes).some((item) => Number(item.value) !== 0)) {
    addIssue(
      context,
      'pptx.image.crop',
      'Picture cropping',
      `Crop settings for “${name}” will be normalized on export.`
    );
  }
  return {
    ...base,
    fill: 'transparent',
    image: {
      dataUrl: bytesToDataUrl(bytes, contentType),
      contentType,
      name,
    },
  };
}

async function parseGraphicFrame(
  node: Element,
  context: PptxImportContext,
  transform?: GroupTransform
): Promise<WorkSlideElement> {
  const box = elementBox(node, context, transform);
  const table = firstDescendant(node, 'tbl');
  if (table) {
    const rows = directChildren(table, 'tr').map((row) =>
      directChildren(row, 'tc').map((cell) =>
        directChildren(firstDescendant(cell, 'txBody') ?? cell, 'p')
          .map((paragraph) =>
            descendants(paragraph, 't')
              .map((text) => text.textContent ?? '')
              .join('')
          )
          .join('\n')
      )
    );
    if (
      descendants(table, 'tc').some((cell) =>
        ['gridSpan', 'rowSpan', 'hMerge', 'vMerge'].some((name) => attribute(cell, name))
      )
    ) {
      addIssue(
        context,
        'pptx.table.merge',
        'Merged table cells',
        'Merged table cells are flattened for editing and export.'
      );
    }
    return {
      id: createWorkId('element'),
      type: 'table',
      ...box,
      text: '',
      fontSize: 12,
      color: '#172033',
      fill: '#ffffff',
      bold: false,
      align: 'left',
      borderColor: '#cbd2de',
      borderWidth: 1,
      table: { rows, headerRows: rows.length > 1 ? 1 : 0 },
      altText: readAlternativeText(node),
    };
  }

  const chartReference = firstDescendant(node, 'chart');
  const relationshipId = attribute(chartReference ?? node, 'r:id') ?? attribute(chartReference ?? node, 'id');
  const relationship = relationshipId ? context.relationships.get(relationshipId) : undefined;
  if (relationship?.type.endsWith('/chart') && context.archive.has(relationship.target)) {
    const result = readPptxChart(await context.archive.xml(relationship.target));
    for (const diagnostic of result.diagnostics) {
      addIssue(context, diagnostic.code, diagnostic.feature, diagnostic.message);
    }
    const chart = result.chart;
    return {
      id: createWorkId('element'),
      type: 'chart',
      ...box,
      text: chart.title ?? '',
      fontSize: 12,
      color: '#172033',
      fill: '#ffffff',
      bold: false,
      align: 'center',
      borderColor: '#d9dee8',
      borderWidth: 1,
      chart,
      altText: readAlternativeText(node),
    };
  }

  addIssue(
    context,
    'pptx.graphic-frame',
    'SmartArt and diagrams',
    'A SmartArt or diagram object remains in the original PPTX only.'
  );
  return {
    id: createWorkId('element'),
    type: 'shape',
    ...box,
    text: 'SmartArt / 图示',
    fontSize: 14,
    color: '#536078',
    fill: '#eef1f6',
    bold: true,
    align: 'center',
    shapeType: 'rect',
  };
}

function parseConnector(node: Element, context: PptxImportContext, transform?: GroupTransform): WorkSlideElement {
  const box = elementBox(node, context, transform);
  const properties = directChild(node, 'spPr');
  const line = firstDescendant(properties, 'ln');
  const color = readPptxFill(line, context.theme, '#657087');
  return {
    id: createWorkId('element'),
    type: 'line',
    ...box,
    text: '',
    fontSize: 12,
    color: color.color,
    fill: 'transparent',
    bold: false,
    align: 'left',
    shapeType: 'line',
    borderColor: color.color,
    borderWidth: numberAttribute(line, 'w', 12_700) / 12_700,
  };
}

async function parseGroup(
  node: Element,
  context: PptxImportContext,
  parentTransform?: GroupTransform
): Promise<WorkSlideElement[]> {
  addIssue(
    context,
    'pptx.group',
    'Grouped objects',
    'Grouped objects are imported as independently editable elements.'
  );
  const xfrm = childPath(node, 'grpSpPr', 'xfrm');
  const offset = directChild(xfrm ?? node, 'off');
  const extent = directChild(xfrm ?? node, 'ext');
  const childOffset = directChild(xfrm ?? node, 'chOff');
  const childExtent = directChild(xfrm ?? node, 'chExt');
  const group: PptxRawBox = {
    x: numberAttribute(offset, 'x', 0),
    y: numberAttribute(offset, 'y', 0),
    width: numberAttribute(extent, 'cx', context.slideWidthEmu),
    height: numberAttribute(extent, 'cy', context.slideHeightEmu),
  };
  const viewport: PptxRawBox = {
    x: numberAttribute(childOffset, 'x', 0),
    y: numberAttribute(childOffset, 'y', 0),
    width: numberAttribute(childExtent, 'cx', group.width),
    height: numberAttribute(childExtent, 'cy', group.height),
  };
  const transform: GroupTransform = {
    map: (box) => {
      const mapped = {
        ...box,
        x: group.x + ((box.x - viewport.x) * group.width) / Math.max(1, viewport.width),
        y: group.y + ((box.y - viewport.y) * group.height) / Math.max(1, viewport.height),
        width: (box.width * group.width) / Math.max(1, viewport.width),
        height: (box.height * group.height) / Math.max(1, viewport.height),
      };
      return parentTransform ? parentTransform.map(mapped) : mapped;
    },
  };
  const elements: WorkSlideElement[] = [];
  for (const child of directChildren(node)) {
    elements.push(...(await parseSlideNode(child, context, transform)));
  }
  return elements;
}

function parseText(body: Element | undefined, context: PptxImportContext): ParsedText {
  const runs: WorkSlideTextRun[] = [];
  const paragraphs = body ? directChildren(body, 'p') : [];
  let align: WorkSlideTextAlign = 'left';
  let explicitStyle = false;
  for (const [paragraphIndex, paragraph] of paragraphs.entries()) {
    const paragraphProperties = directChild(paragraph, 'pPr');
    if (paragraphProperties && (paragraphProperties.attributes.length || paragraphProperties.children.length)) {
      explicitStyle = true;
    }
    if (paragraphIndex === 0) align = pptxTextAlignment(attribute(paragraphProperties ?? paragraph, 'algn'));
    const bullet = directChild(paragraphProperties ?? paragraph, 'buChar');
    const autoNumber = directChild(paragraphProperties ?? paragraph, 'buAutoNum');
    if (bullet || autoNumber) {
      explicitStyle = true;
      runs.push({ text: bullet ? `${attribute(bullet, 'char') ?? '•'} ` : `${paragraphIndex + 1}. ` });
    }
    for (const child of directChildren(paragraph)) {
      if (child.localName === 'br') {
        runs.push({ text: '\n' });
        continue;
      }
      if (child.localName !== 'r' && child.localName !== 'fld') continue;
      const text = directChild(child, 't')?.textContent ?? '';
      if (!text) continue;
      const runProperties = directChild(child, 'rPr') ?? directChild(paragraphProperties ?? paragraph, 'defRPr');
      if (runProperties) explicitStyle = true;
      const color = runProperties ? readPptxFill(runProperties, context.theme, '#172033').color : undefined;
      const hyperlink = firstDescendant(runProperties, 'hlinkClick');
      const relationshipId = attribute(hyperlink ?? child, 'r:id') ?? attribute(hyperlink ?? child, 'id');
      const relationship = relationshipId ? context.relationships.get(relationshipId) : undefined;
      runs.push({
        text,
        fontSize: runProperties ? numberAttribute(runProperties, 'sz', 1800) / 100 : undefined,
        color,
        bold: runProperties ? booleanAttribute(runProperties, 'b') : undefined,
        italic: runProperties ? booleanAttribute(runProperties, 'i') : undefined,
        underline: runProperties
          ? Boolean(attribute(runProperties, 'u') && attribute(runProperties, 'u') !== 'none')
          : undefined,
        fontFamily: runProperties
          ? (attribute(directChild(runProperties, 'latin') ?? runProperties, 'typeface') ??
            attribute(directChild(runProperties, 'ea') ?? runProperties, 'typeface') ??
            undefined)
          : undefined,
        href: relationship?.targetMode === 'External' ? relationship.target : undefined,
      });
    }
    if (paragraphIndex < paragraphs.length - 1) runs.push({ text: '\n' });
  }
  const first = runs.find((run) => run.text !== '\n');
  const bodyProperties = body ? directChild(body, 'bodyPr') : undefined;
  if (bodyProperties?.attributes.length) explicitStyle = true;
  return {
    text: runs.map((run) => run.text).join(''),
    runs,
    align,
    verticalAlign: pptxVerticalAlignment(bodyProperties ? attribute(bodyProperties, 'anchor') : null),
    fontSize: first?.fontSize ?? 18,
    color: first?.color ?? '#172033',
    bold: first?.bold ?? false,
    italic: first?.italic ?? false,
    underline: first?.underline ?? false,
    fontFamily: first?.fontFamily,
    href: runs.find((run) => run.href)?.href,
    explicitStyle,
  };
}

function elementBox(node: Element, context: PptxImportContext, transform?: GroupTransform) {
  const raw = pptxRawElementBox(node) ??
    pptxPlaceholderBox(node, context.placeholders) ?? {
      x: context.slideWidthEmu * 0.1,
      y: context.slideHeightEmu * 0.1,
      width: context.slideWidthEmu * 0.4,
      height: context.slideHeightEmu * 0.15,
    };
  const mapped = transform ? transform.map(raw) : raw;
  return {
    x: (mapped.x / context.slideWidthEmu) * 100,
    y: (mapped.y / context.slideHeightEmu) * 100,
    width: Math.max(0.5, (mapped.width / context.slideWidthEmu) * 100),
    height: Math.max(0.5, (mapped.height / context.slideHeightEmu) * 100),
    rotation: mapped.rotation,
  };
}

async function readSpeakerNotes(
  archive: OoxmlPackage,
  relationships: Map<string, OoxmlRelationship>,
  context: PptxImportContext
): Promise<string | undefined> {
  const relationship = Array.from(relationships.values()).find((item) => item.type.endsWith('/notesSlide'));
  if (!relationship || !archive.has(relationship.target)) return undefined;
  const document = await archive.xml(relationship.target);
  const bodies = descendants(document, 'sp')
    .filter((shape) => attribute(firstDescendant(directChild(shape, 'nvSpPr'), 'ph') ?? shape, 'type') === 'body')
    .map((shape) => parseText(firstDescendant(shape, 'txBody'), context).text.trim())
    .filter(Boolean);
  return bodies.join('\n\n') || undefined;
}

function orderedSlideParts(presentation: Document, relationships: Map<string, OoxmlRelationship>): string[] {
  return descendants(presentation, 'sldId')
    .map((node) => {
      const id = attribute(node, 'r:id') ?? attribute(node, 'id');
      return id ? relationships.get(id) : undefined;
    })
    .filter((relationship): relationship is OoxmlRelationship => Boolean(relationship?.type.endsWith('/slide')))
    .map((relationship) => relationship.target);
}

function readSlideName(document: Document, slideNumber: number): string {
  const titleShape = descendants(document, 'sp').find((shape) => {
    const type = attribute(firstDescendant(directChild(shape, 'nvSpPr'), 'ph') ?? shape, 'type');
    return type === 'title' || type === 'ctrTitle';
  });
  const title = titleShape
    ? descendants(titleShape, 't')
        .map((node) => node.textContent ?? '')
        .join('')
        .trim()
    : '';
  return title || `幻灯片 ${slideNumber}`;
}

function readAlternativeText(node: Element): string | undefined {
  const properties = firstDescendant(node, 'cNvPr');
  return attribute(properties ?? node, 'descr')?.trim() || attribute(properties ?? node, 'title')?.trim() || undefined;
}

function addIssue(
  context: PptxImportContext,
  code: string,
  feature: string,
  message: string,
  severity: WorkCompatibilityIssue['severity'] = 'warning'
) {
  context.issues.push({
    code,
    severity,
    feature,
    message,
    location: context.location ?? `幻灯片 ${context.slideNumber}`,
  });
}

function addFontDiagnostics(slides: WorkSlide[], issues: WorkCompatibilityIssue[]) {
  const fonts = new Set<string>();
  for (const slide of slides) {
    for (const element of slide.elements) {
      if (element.fontFamily) fonts.add(element.fontFamily);
      for (const run of element.textRuns ?? []) if (run.fontFamily) fonts.add(run.fontFamily);
    }
  }
  for (const font of fonts) {
    if (font.startsWith('+m') || font.startsWith('+mj') || font.startsWith('+mn')) continue;
    const available = typeof document.fonts?.check === 'function' ? document.fonts.check(`12px "${font}"`) : true;
    if (!available) {
      issues.push({
        code: `pptx.font.${font.toLowerCase()}`,
        severity: 'warning',
        feature: 'Font substitution',
        message: `The font “${font}” is unavailable in this browser and may be substituted in preview and export.`,
      });
    }
  }
}

function deduplicateIssues(issues: WorkCompatibilityIssue[]): WorkCompatibilityIssue[] {
  const seen = new Set<string>();
  return issues.filter((issue) => {
    const key = `${issue.code}:${issue.location ?? ''}:${issue.message}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function booleanAttribute(element: Element | undefined, name: string): boolean {
  if (!element) return false;
  const value = attribute(element, name);
  return value === '1' || value === 'true';
}

function numberAttribute(element: Element | undefined, name: string, fallback: number): number {
  if (!element) return fallback;
  const value = Number(attribute(element, name));
  return Number.isFinite(value) ? value : fallback;
}

function positiveNumber(value: string | null): number | undefined {
  const number = Number(value);
  return Number.isFinite(number) && number > 0 ? number : undefined;
}
