import JSZip from 'jszip';
import { attribute, directChild, directChildren, OoxmlPackage, parseXml } from './work-ooxml-package';
import {
  createDocumentBibliography,
  documentCitationStyle,
  documentCitationStyleDetails,
} from './work-document-citations';
import type {
  WorkDocumentBibliography,
  WorkDocumentCitationContributor,
  WorkDocumentCitationPerson,
  WorkDocumentCitationSource,
  WorkDocumentCitationStyle,
} from './work-types';

export interface DocxBibliographyReadResult {
  bibliography?: WorkDocumentBibliography;
  sourcePartCount: number;
  unreadablePartCount: number;
  duplicateTags: string[];
  uncommonSourceTypes: string[];
  uncommonStyle?: string;
}

const BIBLIOGRAPHY_NAMESPACE = 'http://schemas.openxmlformats.org/officeDocument/2006/bibliography';
const CUSTOM_XML_NAMESPACE = 'http://schemas.openxmlformats.org/officeDocument/2006/customXml';
const OFFICE_RELATIONSHIPS_NAMESPACE = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
const PACKAGE_RELATIONSHIPS_NAMESPACE = 'http://schemas.openxmlformats.org/package/2006/relationships';
const CONTENT_TYPES_NAMESPACE = 'http://schemas.openxmlformats.org/package/2006/content-types';
const CUSTOM_XML_RELATIONSHIP = `${OFFICE_RELATIONSHIPS_NAMESPACE}/customXml`;
const CUSTOM_XML_PROPS_RELATIONSHIP = `${OFFICE_RELATIONSHIPS_NAMESPACE}/customXmlProps`;
const CUSTOM_XML_PROPS_CONTENT_TYPE = 'application/vnd.openxmlformats-officedocument.customXmlProperties+xml';
const COMMON_SOURCE_TYPES = new Set([
  'Book',
  'BookSection',
  'JournalArticle',
  'ArticleInAPeriodical',
  'ConferenceProceedings',
  'Report',
  'InternetSite',
  'DocumentFromInternetSite',
  'ElectronicSource',
  'Misc',
]);
const RESERVED_FIELDS = new Set([
  'Tag',
  'SourceType',
  'Guid',
  'Author',
  'Title',
  'Year',
  'Publisher',
  'City',
  'JournalName',
  'Volume',
  'Issue',
  'Pages',
  'URL',
  'StandardNumber',
  'ConferenceName',
  'Institution',
]);

export async function readDocxBibliography(archive: OoxmlPackage): Promise<DocxBibliographyReadResult> {
  const paths = archive.paths('customXml/').filter((path) => /^customXml\/item(?!Props)[^/]*\.xml$/i.test(path));
  const sources: WorkDocumentCitationSource[] = [];
  const usedTags = new Set<string>();
  const duplicateTags = new Set<string>();
  const uncommonSourceTypes = new Set<string>();
  let sourcePartCount = 0;
  let unreadablePartCount = 0;
  let selectedStyle = '';
  let styleName = '';
  for (const path of paths) {
    try {
      const document = await archive.xml(path);
      if (
        document.documentElement.localName !== 'Sources' ||
        document.documentElement.namespaceURI !== BIBLIOGRAPHY_NAMESPACE
      ) {
        continue;
      }
      sourcePartCount += 1;
      selectedStyle ||= attribute(document.documentElement, 'SelectedStyle')?.trim() ?? '';
      styleName ||= attribute(document.documentElement, 'StyleName')?.trim() ?? '';
      for (const element of directChildren(document.documentElement, 'Source')) {
        const source = parseCitationSource(element, sources.length + 1);
        if (usedTags.has(source.tag)) {
          duplicateTags.add(source.tag);
          source.tag = uniqueSourceTag(source.tag, usedTags);
        }
        usedTags.add(source.tag);
        if (!COMMON_SOURCE_TYPES.has(source.sourceType)) uncommonSourceTypes.add(source.sourceType);
        sources.push(source);
      }
    } catch {
      unreadablePartCount += 1;
    }
  }
  if (!sourcePartCount) {
    return {
      sourcePartCount,
      unreadablePartCount,
      duplicateTags: [],
      uncommonSourceTypes: [],
      uncommonStyle: undefined,
    };
  }
  const styleResult = bibliographyStyle(selectedStyle, styleName);
  const style = styleResult.style;
  return {
    bibliography: {
      ...createDocumentBibliography(style),
      selectedStyle: selectedStyle || documentCitationStyleDetails(style).selectedStyle,
      styleName: styleName || documentCitationStyleDetails(style).name,
      sources,
    },
    sourcePartCount,
    unreadablePartCount,
    duplicateTags: Array.from(duplicateTags),
    uncommonSourceTypes: Array.from(uncommonSourceTypes),
    uncommonStyle: styleResult.recognized ? undefined : styleName || selectedStyle,
  };
}

export async function patchDocxBibliography(
  buffer: ArrayBuffer,
  bibliography: WorkDocumentBibliography | undefined
): Promise<ArrayBuffer> {
  if (!bibliography?.sources.length) return buffer;
  const archive = await JSZip.loadAsync(buffer);
  const itemId = stableGuid(`bibliography:${bibliography.sources.map((source) => source.tag).join('|')}`);
  archive.file('customXml/item1.xml', serializeBibliographySources(bibliography));
  archive.file('customXml/itemProps1.xml', serializeCustomXmlProperties(itemId));
  archive.file('customXml/_rels/item1.xml.rels', serializeCustomXmlRelationships());
  await upsertRelationship(archive, 'word/document.xml', CUSTOM_XML_RELATIONSHIP, '../customXml/item1.xml');
  await upsertContentType(archive, '/customXml/itemProps1.xml', CUSTOM_XML_PROPS_CONTENT_TYPE);
  return archive.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });
}

function parseCitationSource(element: Element, index: number): WorkDocumentCitationSource {
  const simple = new Map(
    directChildren(element)
      .filter((child) => !child.children.length)
      .map((child) => [child.localName, child.textContent?.trim() ?? ''] as const)
  );
  const tag = simple.get('Tag') || `Source${index}`;
  const additionalFields: Record<string, string> = {};
  for (const [name, value] of simple) {
    if (!RESERVED_FIELDS.has(name) && value) additionalFields[name] = value;
  }
  return {
    id: `docx-source-${safeIdPart(tag)}-${index}`,
    tag,
    sourceType: simple.get('SourceType') || 'Misc',
    guid: simple.get('Guid') || undefined,
    title: simple.get('Title') || '',
    year: simple.get('Year') || undefined,
    contributors: parseCitationContributors(directChild(element, 'Author')),
    publisher: simple.get('Publisher') || undefined,
    city: simple.get('City') || undefined,
    journalName: simple.get('JournalName') || undefined,
    volume: simple.get('Volume') || undefined,
    issue: simple.get('Issue') || undefined,
    pages: simple.get('Pages') || undefined,
    url: simple.get('URL') || undefined,
    standardNumber: simple.get('StandardNumber') || undefined,
    conferenceName: simple.get('ConferenceName') || undefined,
    institution: simple.get('Institution') || undefined,
    additionalFields: Object.keys(additionalFields).length ? additionalFields : undefined,
  };
}

function parseCitationContributors(
  wrapper: Element | undefined
): Record<string, WorkDocumentCitationContributor> | undefined {
  if (!wrapper) return undefined;
  const roles =
    directChild(wrapper, 'NameList') || directChild(wrapper, 'Corporate') ? [wrapper] : directChildren(wrapper);
  const contributors: Record<string, WorkDocumentCitationContributor> = {};
  for (const role of roles) {
    const nameList = directChild(role, 'NameList');
    const people = nameList
      ? directChildren(nameList, 'Person').map(parseCitationPerson).filter(validCitationPerson)
      : [];
    const corporate = directChild(role, 'Corporate')?.textContent?.trim();
    if (people.length || corporate) {
      contributors[role === wrapper ? 'Author' : role.localName] = {
        people: people.length ? people : undefined,
        corporate: corporate || undefined,
      };
    }
  }
  return Object.keys(contributors).length ? contributors : undefined;
}

function parseCitationPerson(element: Element): WorkDocumentCitationPerson {
  return {
    first: directChild(element, 'First')?.textContent?.trim() ?? '',
    middle: directChild(element, 'Middle')?.textContent?.trim() || undefined,
    last: directChild(element, 'Last')?.textContent?.trim() ?? '',
    suffix: directChild(element, 'Suffix')?.textContent?.trim() || undefined,
  };
}

function validCitationPerson(person: WorkDocumentCitationPerson): boolean {
  return Boolean(person.first || person.middle || person.last || person.suffix);
}

function serializeBibliographySources(bibliography: WorkDocumentBibliography): string {
  const styleDetails = documentCitationStyleDetails(bibliography.style);
  const document = parseXml(
    `<b:Sources xmlns:b="${BIBLIOGRAPHY_NAMESPACE}" xmlns="${BIBLIOGRAPHY_NAMESPACE}"/>`,
    'DOCX bibliography sources'
  );
  const root = document.documentElement;
  root.setAttribute('SelectedStyle', bibliography.selectedStyle || styleDetails.selectedStyle);
  root.setAttribute('StyleName', bibliography.styleName || styleDetails.name);
  root.setAttribute('Version', '6');
  for (const source of bibliography.sources) {
    const element = bibliographyElement(document, 'Source');
    appendTextElement(document, element, 'Tag', source.tag);
    appendTextElement(document, element, 'SourceType', source.sourceType || 'Misc');
    appendTextElement(
      document,
      element,
      'Guid',
      normalizeGuid(source.guid) || stableGuid(`source:${source.id}:${source.tag}`)
    );
    appendContributors(document, element, source.contributors);
    appendTextElement(document, element, 'Title', source.title);
    appendTextElement(document, element, 'Year', source.year);
    appendTextElement(document, element, 'Publisher', source.publisher);
    appendTextElement(document, element, 'City', source.city);
    appendTextElement(document, element, 'JournalName', source.journalName);
    appendTextElement(document, element, 'Volume', source.volume);
    appendTextElement(document, element, 'Issue', source.issue);
    appendTextElement(document, element, 'Pages', source.pages);
    appendTextElement(document, element, 'URL', source.url);
    appendTextElement(document, element, 'StandardNumber', source.standardNumber);
    appendTextElement(document, element, 'ConferenceName', source.conferenceName);
    appendTextElement(document, element, 'Institution', source.institution);
    for (const [name, value] of Object.entries(source.additionalFields ?? {}).sort(([left], [right]) =>
      left.localeCompare(right)
    )) {
      if (!RESERVED_FIELDS.has(name) && /^[A-Za-z_][A-Za-z0-9_.-]*$/.test(name)) {
        appendTextElement(document, element, name, value);
      }
    }
    root.append(element);
  }
  return new XMLSerializer().serializeToString(document);
}

function appendContributors(
  document: Document,
  source: Element,
  contributors: Record<string, WorkDocumentCitationContributor> | undefined
): void {
  const entries = Object.entries(contributors ?? {}).filter(
    ([, contributor]) => contributor.people?.length || contributor.corporate?.trim()
  );
  if (!entries.length) return;
  const wrapper = bibliographyElement(document, 'Author');
  for (const [roleName, contributor] of entries) {
    if (!/^[A-Za-z_][A-Za-z0-9_.-]*$/.test(roleName)) continue;
    const role = bibliographyElement(document, roleName);
    if (contributor.people?.length) {
      const nameList = bibliographyElement(document, 'NameList');
      for (const person of contributor.people) {
        const element = bibliographyElement(document, 'Person');
        appendTextElement(document, element, 'Last', person.last);
        appendTextElement(document, element, 'First', person.first);
        appendTextElement(document, element, 'Middle', person.middle);
        appendTextElement(document, element, 'Suffix', person.suffix);
        nameList.append(element);
      }
      role.append(nameList);
    } else if (contributor.corporate?.trim()) {
      appendTextElement(document, role, 'Corporate', contributor.corporate);
    }
    wrapper.append(role);
  }
  if (wrapper.children.length) source.append(wrapper);
}

function appendTextElement(document: Document, parent: Element, name: string, value: string | undefined): void {
  if (!value?.trim()) return;
  const element = bibliographyElement(document, name);
  element.textContent = value.trim();
  parent.append(element);
}

function bibliographyElement(document: Document, name: string): Element {
  return document.createElementNS(BIBLIOGRAPHY_NAMESPACE, `b:${name}`);
}

function serializeCustomXmlProperties(itemId: string): string {
  const document = parseXml(
    `<ds:datastoreItem xmlns:ds="${CUSTOM_XML_NAMESPACE}" ds:itemID="${itemId}"/>`,
    'DOCX custom XML properties'
  );
  const references = document.createElementNS(CUSTOM_XML_NAMESPACE, 'ds:schemaRefs');
  const reference = document.createElementNS(CUSTOM_XML_NAMESPACE, 'ds:schemaRef');
  reference.setAttributeNS(CUSTOM_XML_NAMESPACE, 'ds:uri', BIBLIOGRAPHY_NAMESPACE);
  references.append(reference);
  document.documentElement.append(references);
  return new XMLSerializer().serializeToString(document);
}

function serializeCustomXmlRelationships(): string {
  return [
    `<Relationships xmlns="${PACKAGE_RELATIONSHIPS_NAMESPACE}">`,
    `<Relationship Id="rId1" Type="${CUSTOM_XML_PROPS_RELATIONSHIP}" Target="itemProps1.xml"/>`,
    '</Relationships>',
  ].join('');
}

async function upsertRelationship(archive: JSZip, sourcePart: string, type: string, target: string): Promise<void> {
  const path = relationshipPartPath(sourcePart);
  const entry = archive.file(path);
  const document = entry
    ? parseXml(await entry.async('text'), path)
    : parseXml(`<Relationships xmlns="${PACKAGE_RELATIONSHIPS_NAMESPACE}"/>`, path);
  const root = document.documentElement;
  const existing = directChildren(root, 'Relationship').find((item) => attribute(item, 'Type') === type);
  if (existing) {
    existing.setAttribute('Target', target);
  } else {
    const relationship = document.createElementNS(PACKAGE_RELATIONSHIPS_NAMESPACE, 'Relationship');
    relationship.setAttribute('Id', nextRelationshipId(root));
    relationship.setAttribute('Type', type);
    relationship.setAttribute('Target', target);
    root.append(relationship);
  }
  archive.file(path, new XMLSerializer().serializeToString(document));
}

async function upsertContentType(archive: JSZip, partName: string, contentType: string): Promise<void> {
  const path = '[Content_Types].xml';
  const entry = archive.file(path);
  if (!entry) throw new Error('DOCX content types part is missing.');
  const document = parseXml(await entry.async('text'), path);
  const existing = directChildren(document.documentElement, 'Override').find(
    (item) => attribute(item, 'PartName') === partName
  );
  if (existing) {
    existing.setAttribute('ContentType', contentType);
  } else {
    const override = document.createElementNS(CONTENT_TYPES_NAMESPACE, 'Override');
    override.setAttribute('PartName', partName);
    override.setAttribute('ContentType', contentType);
    document.documentElement.append(override);
  }
  archive.file(path, new XMLSerializer().serializeToString(document));
}

function bibliographyStyle(
  selectedStyle: string,
  styleName: string
): { style: WorkDocumentCitationStyle; recognized: boolean } {
  const value = `${selectedStyle} ${styleName}`.toLowerCase();
  if (value.includes('ieee')) return { style: 'ieee', recognized: true };
  if (value.includes('mla')) return { style: 'mla', recognized: true };
  if (value.includes('chicago')) return { style: 'chicago', recognized: true };
  if (!value.trim() || value.includes('apa')) {
    return { style: documentCitationStyle('apa'), recognized: true };
  }
  return { style: documentCitationStyle('apa'), recognized: false };
}

function relationshipPartPath(sourcePart: string): string {
  const separator = sourcePart.lastIndexOf('/');
  const directory = separator >= 0 ? sourcePart.slice(0, separator + 1) : '';
  const fileName = separator >= 0 ? sourcePart.slice(separator + 1) : sourcePart;
  return `${directory}_rels/${fileName}.rels`;
}

function nextRelationshipId(root: Element): string {
  const used = new Set(directChildren(root, 'Relationship').map((item) => attribute(item, 'Id') ?? ''));
  let index = 1;
  while (used.has(`rId${index}`)) index += 1;
  return `rId${index}`;
}

function uniqueSourceTag(base: string, used: Set<string>): string {
  let index = 2;
  while (used.has(`${base}_${index}`)) index += 1;
  return `${base}_${index}`;
}

function safeIdPart(value: string): string {
  return (
    value
      .replace(/[^a-z0-9_-]/gi, '-')
      .replace(/-+/g, '-')
      .replace(/^-|-$/g, '') || 'source'
  );
}

function normalizeGuid(value: string | undefined): string {
  const match = /^\{?([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\}?$/i.exec(value?.trim() ?? '');
  return match ? `{${match[1].toUpperCase()}}` : '';
}

function stableGuid(source: string): string {
  const values = [0, 1, 2, 3].map((salt) => stableHash(`${salt}:${source}`));
  const hex = values.join('').slice(0, 32).split('');
  hex[12] = '4';
  hex[16] = ['8', '9', 'A', 'B'][Number.parseInt(hex[16] ?? '0', 16) % 4];
  const value = hex.join('').toUpperCase();
  return `{${value.slice(0, 8)}-${value.slice(8, 12)}-${value.slice(12, 16)}-${value.slice(
    16,
    20
  )}-${value.slice(20)}}`;
}

function stableHash(source: string): string {
  let hash = 2_166_136_261;
  for (let index = 0; index < source.length; index += 1) {
    hash ^= source.charCodeAt(index);
    hash = Math.imul(hash, 16_777_619);
  }
  return (hash >>> 0).toString(16).padStart(8, '0');
}
