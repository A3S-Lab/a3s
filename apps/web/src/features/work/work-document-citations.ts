import type {
  WorkDocumentBibliography,
  WorkDocumentCitationContributor,
  WorkDocumentCitationPerson,
  WorkDocumentCitationSource,
  WorkDocumentCitationStyle,
} from './work-types';

export interface ResolvedDocumentCitation {
  text: string;
  orphaned: boolean;
}

const CITATION_SELECTOR = 'span[data-document-citation]';
const BIBLIOGRAPHY_SELECTOR = 'section[data-document-bibliography]';

const STYLE_DETAILS: Record<WorkDocumentCitationStyle, { name: string; selectedStyle: string }> = {
  apa: { name: 'APA', selectedStyle: '\\APASixthEditionOfficeOnline.xsl' },
  mla: { name: 'MLA', selectedStyle: '\\MLASeventhEditionOfficeOnline.xsl' },
  chicago: { name: 'Chicago', selectedStyle: '\\CHICAGO.XSL' },
  ieee: { name: 'IEEE', selectedStyle: '\\IEEE.XSL' },
};

export function createDocumentBibliography(style: WorkDocumentCitationStyle = 'apa'): WorkDocumentBibliography {
  return {
    style,
    styleName: STYLE_DETAILS[style].name,
    selectedStyle: STYLE_DETAILS[style].selectedStyle,
    sources: [],
  };
}

export function documentCitationStyle(value: string | undefined): WorkDocumentCitationStyle {
  if (value === 'mla' || value === 'chicago' || value === 'ieee') return value;
  return 'apa';
}

export function documentCitationStyleDetails(style: WorkDocumentCitationStyle): {
  name: string;
  selectedStyle: string;
} {
  return STYLE_DETAILS[style];
}

export function documentCitationTagsFromInstruction(instruction: string): string[] {
  const primary = /^\s*CITATION\s+(?:"([^"]+)"|([^\s\\]+))/i.exec(instruction);
  if (!primary) return [];
  const tags = [primary[1] || primary[2]];
  const additional = /\\m\s+(?:"([^"]+)"|([^\s\\]+))/gi;
  for (const match of instruction.matchAll(additional)) tags.push(match[1] || match[2]);
  return uniqueCitationTags(tags);
}

export function documentCitationInstruction(tags: readonly string[]): string {
  const normalized = uniqueCitationTags(tags);
  if (!normalized.length) return '';
  return [
    'CITATION',
    citationTagInstructionValue(normalized[0]),
    ...normalized.slice(1).flatMap((tag) => ['\\m', citationTagInstructionValue(tag)]),
    '\\l',
    '2052',
  ].join(' ');
}

export function renameDocumentCitationTagInInstruction(
  instruction: string,
  previousTag: string,
  nextTag: string
): string {
  return instruction.replace(
    /(^\s*CITATION\s+|\\m\s+)("[^"]+"|[^\s\\]+)/gi,
    (match, prefix: string, source: string) => {
      const tag = source.startsWith('"') ? source.slice(1, -1) : source;
      return tag === previousTag ? `${prefix}${citationTagInstructionValue(nextTag)}` : match;
    }
  );
}

export function documentCitationTags(value: string | undefined): string[] {
  return uniqueCitationTags((value ?? '').split(/\s+/));
}

export function isValidDocumentCitationTag(value: string): boolean {
  return /^[A-Za-z0-9_:.+-]{1,80}$/.test(value);
}

export function resolveDocumentCitation(
  tags: readonly string[],
  bibliography: WorkDocumentBibliography | undefined,
  instruction = '',
  cachedValue = ''
): ResolvedDocumentCitation {
  const normalized = uniqueCitationTags(tags);
  if (!normalized.length) return { text: cachedValue || '缺失引文', orphaned: true };
  const sources = new Map((bibliography?.sources ?? []).map((source) => [source.tag, source] as const));
  const missing = normalized.filter((tag) => !sources.has(tag));
  if (missing.length) {
    return {
      text: missing.length === 1 ? `缺失引文：${missing[0]}` : `缺失引文：${missing.join('、')}`,
      orphaned: true,
    };
  }
  const selected = normalized.flatMap((tag) => {
    const source = sources.get(tag);
    return source ? [source] : [];
  });
  const style = bibliography?.style ?? 'apa';
  const suppressAuthor = /(?:^|\s)\\n(?:\s|$)/i.test(instruction);
  const suppressYear = /(?:^|\s)\\v(?:\s|$)/i.test(instruction);
  const prefix = citationSwitch(instruction, 'f');
  const suffix = citationSwitch(instruction, 'p');
  const text =
    style === 'ieee'
      ? ieeeCitation(selected, bibliography?.sources ?? [])
      : style === 'mla'
        ? mlaCitation(selected, suppressAuthor)
        : style === 'chicago'
          ? chicagoCitation(selected, suppressAuthor, suppressYear)
          : apaCitation(selected, suppressAuthor, suppressYear);
  return {
    text: `${prefix}${text}${suffix}`.trim() || cachedValue || normalized.join('; '),
    orphaned: false,
  };
}

export function normalizeDocumentCitationsHtml(
  source: string,
  bibliography: WorkDocumentBibliography | undefined
): string {
  const document = new DOMParser().parseFromString(source, 'text/html');
  const usedIds = new Set<string>();
  for (const [index, element] of Array.from(document.body.querySelectorAll<HTMLElement>(CITATION_SELECTOR)).entries()) {
    const instruction = element.dataset.citationInstruction?.trim() ?? '';
    const tags = documentCitationTags(element.dataset.citationTags);
    const normalizedTags = tags.length ? tags : documentCitationTagsFromInstruction(instruction);
    if (!normalizedTags.length) {
      element.replaceWith(document.createTextNode(element.textContent ?? ''));
      continue;
    }
    const cached = element.dataset.citationDisplay?.trim() || element.textContent?.trim() || '';
    const resolved = resolveDocumentCitation(normalizedTags, bibliography, instruction, cached);
    element.dataset.documentCitation = 'true';
    element.dataset.citationId = uniqueCitationId(element.dataset.citationId, index + 1, usedIds);
    element.dataset.citationTags = normalizedTags.join(' ');
    element.dataset.citationInstruction = instruction || documentCitationInstruction(normalizedTags);
    element.dataset.citationDisplay = resolved.text;
    if (resolved.orphaned) element.dataset.citationOrphaned = 'true';
    else delete element.dataset.citationOrphaned;
    element.classList.add('work-document-citation');
    element.textContent = resolved.text;
  }
  for (const [index, element] of Array.from(
    document.body.querySelectorAll<HTMLElement>(BIBLIOGRAPHY_SELECTOR)
  ).entries()) {
    if (!bibliography) continue;
    const replacement = createBibliographyElement(
      document,
      bibliography,
      element.dataset.bibliographyId || `document-bibliography-${index + 1}`
    );
    element.replaceWith(replacement);
  }
  return document.body.innerHTML;
}

export function renderDocumentBibliographyHtml(
  bibliography: WorkDocumentBibliography,
  id = 'document-bibliography-1'
): string {
  const document = new DOMParser().parseFromString('', 'text/html');
  document.body.append(createBibliographyElement(document, bibliography, id));
  return document.body.innerHTML;
}

export function documentBibliographyEntry(
  source: WorkDocumentCitationSource,
  bibliography: WorkDocumentBibliography,
  index: number
): string {
  const people = primaryCitationContributor(source);
  const authors =
    people?.corporate?.trim() ||
    people?.people?.map((person) => bibliographyPersonName(person, bibliography.style)).join(', ') ||
    '未知作者';
  const title = source.title.trim() || 'Untitled';
  const year = source.year?.trim() || 'n.d.';
  const container = citationContainer(source);
  const url = source.url?.trim();
  if (bibliography.style === 'ieee') {
    return [
      `[${index + 1}] ${authors}, “${title}.”`,
      container ? `${container},` : '',
      year ? `${year}.` : '',
      url ?? '',
    ]
      .filter(Boolean)
      .join(' ');
  }
  if (bibliography.style === 'mla') {
    return [`${authors}.`, `“${title}.”`, container ? `${container},` : '', `${year}.`, url ?? '']
      .filter(Boolean)
      .join(' ');
  }
  if (bibliography.style === 'chicago') {
    return [`${authors}.`, `${year}.`, `“${title}.”`, container ? `${container}.` : '', url ?? '']
      .filter(Boolean)
      .join(' ');
  }
  return [`${authors}.`, `(${year}).`, `${title}.`, container ? `${container}.` : '', url ?? '']
    .filter(Boolean)
    .join(' ');
}

export function primaryCitationContributor(
  source: WorkDocumentCitationSource
): WorkDocumentCitationContributor | undefined {
  return source.contributors?.Author ?? Object.values(source.contributors ?? {})[0];
}

function createBibliographyElement(
  document: Document,
  bibliography: WorkDocumentBibliography,
  id: string
): HTMLElement {
  const section = document.createElement('section');
  section.dataset.documentBibliography = 'true';
  section.dataset.bibliographyId = id;
  section.dataset.bibliographyStyle = bibliography.style;
  section.className = 'work-document-bibliography';
  const heading = document.createElement('h2');
  heading.textContent = '参考文献';
  section.append(heading);
  if (!bibliography.sources.length) {
    const empty = document.createElement('p');
    empty.dataset.bibliographyEmpty = 'true';
    empty.textContent = '尚无文献源';
    section.append(empty);
    return section;
  }
  bibliography.sources.forEach((source, index) => {
    const paragraph = document.createElement('p');
    paragraph.dataset.bibliographyEntry = source.tag;
    paragraph.textContent = documentBibliographyEntry(source, bibliography, index);
    section.append(paragraph);
  });
  return section;
}

function apaCitation(sources: WorkDocumentCitationSource[], suppressAuthor: boolean, suppressYear: boolean): string {
  const items = sources.map((source) => {
    const author = suppressAuthor ? '' : citationAuthor(source, '&');
    const year = suppressYear ? '' : source.year?.trim() || 'n.d.';
    return [author, year].filter(Boolean).join(', ');
  });
  return `(${items.join('; ')})`;
}

function mlaCitation(sources: WorkDocumentCitationSource[], suppressAuthor: boolean): string {
  return `(${sources
    .map((source) => (suppressAuthor ? source.year?.trim() || 'n.d.' : citationAuthor(source, 'and')))
    .join('; ')})`;
}

function chicagoCitation(
  sources: WorkDocumentCitationSource[],
  suppressAuthor: boolean,
  suppressYear: boolean
): string {
  return `(${sources
    .map((source) =>
      [suppressAuthor ? '' : citationAuthor(source, 'and'), suppressYear ? '' : source.year?.trim() || 'n.d.']
        .filter(Boolean)
        .join(' ')
    )
    .join('; ')})`;
}

function ieeeCitation(sources: WorkDocumentCitationSource[], allSources: WorkDocumentCitationSource[]): string {
  const indexes = sources
    .map(
      (source) => allSources.findIndex((candidate) => candidate.id === source.id || candidate.tag === source.tag) + 1
    )
    .filter((index) => index > 0);
  return `[${indexes.join(', ')}]`;
}

function citationAuthor(source: WorkDocumentCitationSource, conjunction: string): string {
  const contributor = primaryCitationContributor(source);
  if (contributor?.corporate?.trim()) return contributor.corporate.trim();
  const names = (contributor?.people ?? []).map((person) => person.last.trim() || person.first.trim()).filter(Boolean);
  if (!names.length) return source.title.trim() || source.tag;
  if (names.length === 1) return names[0];
  if (names.length === 2) return `${names[0]} ${conjunction} ${names[1]}`;
  return `${names[0]} et al.`;
}

function bibliographyPersonName(person: WorkDocumentCitationPerson, style: WorkDocumentCitationStyle): string {
  const first = [person.first, person.middle].filter(Boolean).join(' ').trim();
  const suffix = person.suffix?.trim();
  if (style === 'ieee') {
    const initials = [person.first, person.middle]
      .filter(Boolean)
      .map((value) => `${Array.from(value ?? '')[0] ?? ''}.`)
      .join(' ');
    return [initials, person.last, suffix].filter(Boolean).join(' ');
  }
  return [person.last, first ? `, ${first}` : '', suffix ? `, ${suffix}` : ''].join('');
}

function citationContainer(source: WorkDocumentCitationSource): string {
  return (
    source.journalName?.trim() ||
    source.publisher?.trim() ||
    source.conferenceName?.trim() ||
    source.institution?.trim() ||
    ''
  );
}

function citationSwitch(instruction: string, name: string): string {
  const expression = new RegExp(`\\\\${name}\\s+"([^"]*)"`, 'i');
  return expression.exec(instruction)?.[1] ?? '';
}

function citationTagInstructionValue(tag: string): string {
  return /^[A-Za-z0-9_:.+-]+$/.test(tag) ? tag : `"${tag.replaceAll('"', '')}"`;
}

function uniqueCitationTags(tags: readonly string[]): string[] {
  const result: string[] = [];
  const seen = new Set<string>();
  for (const source of tags) {
    const tag = source.trim();
    if (!tag || seen.has(tag)) continue;
    seen.add(tag);
    result.push(tag);
  }
  return result;
}

function uniqueCitationId(source: string | undefined, index: number, usedIds: Set<string>): string {
  const candidate = source?.trim();
  if (candidate && !usedIds.has(candidate)) {
    usedIds.add(candidate);
    return candidate;
  }
  let suffix = index;
  while (usedIds.has(`document-citation-${suffix}`)) suffix += 1;
  const id = `document-citation-${suffix}`;
  usedIds.add(id);
  return id;
}
