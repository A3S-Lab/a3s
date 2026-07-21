import JSZip from 'jszip';

export interface OoxmlRelationship {
  id: string;
  target: string;
  type: string;
  targetMode?: string;
}

export class OoxmlPackage {
  private constructor(private readonly zip: JSZip) {}

  static async load(buffer: ArrayBuffer): Promise<OoxmlPackage> {
    return new OoxmlPackage(await JSZip.loadAsync(buffer));
  }

  has(partPath: string): boolean {
    return Boolean(this.zip.file(partPath));
  }

  paths(prefix: string): string[] {
    return Object.keys(this.zip.files).filter((path) => path.startsWith(prefix) && !this.zip.files[path]?.dir);
  }

  async text(partPath: string): Promise<string> {
    const entry = this.zip.file(partPath);
    if (!entry) throw new Error(`Office package part is missing: ${partPath}`);
    return entry.async('text');
  }

  async xml(partPath: string): Promise<Document> {
    return parseXml(await this.text(partPath), partPath);
  }

  async bytes(partPath: string): Promise<Uint8Array> {
    const entry = this.zip.file(partPath);
    if (!entry) throw new Error(`Office package part is missing: ${partPath}`);
    return entry.async('uint8array');
  }

  async relationships(sourcePart: string): Promise<Map<string, OoxmlRelationship>> {
    const partPath = relationshipsPartPath(sourcePart);
    if (!this.has(partPath)) return new Map();
    const document = await this.xml(partPath);
    return new Map(
      descendants(document, 'Relationship').map((element) => {
        const relationship: OoxmlRelationship = {
          id: attribute(element, 'Id') ?? '',
          target: resolvePartTarget(sourcePart, attribute(element, 'Target') ?? ''),
          type: attribute(element, 'Type') ?? '',
          targetMode: attribute(element, 'TargetMode') ?? undefined,
        };
        return [relationship.id, relationship];
      })
    );
  }
}

export function parseXml(source: string, label = 'Office XML'): Document {
  const document = new DOMParser().parseFromString(source, 'application/xml');
  const error = descendants(document, 'parsererror')[0];
  if (error) throw new Error(`${label} is not valid XML: ${error.textContent?.trim() || 'parse error'}`);
  return document;
}

export function attribute(element: Element, name: string): string | null {
  const direct = element.getAttribute(name);
  if (direct !== null) return direct;
  const localName = name.includes(':') ? name.slice(name.indexOf(':') + 1) : name;
  return (
    Array.from(element.attributes).find(
      (item) => item.localName === localName && (!name.includes(':') || item.name === name)
    )?.value ?? null
  );
}

export function directChildren(parent: ParentNode, localName?: string): Element[] {
  return Array.from(parent.children).filter((element) => !localName || element.localName === localName);
}

export function directChild(parent: ParentNode, localName: string): Element | undefined {
  return directChildren(parent, localName)[0];
}

export function descendants(parent: ParentNode, localName: string): Element[] {
  return Array.from(parent.querySelectorAll('*')).filter((element) => element.localName === localName);
}

export function firstDescendant(parent: ParentNode | null | undefined, localName: string): Element | undefined {
  if (!parent) return undefined;
  return descendants(parent, localName)[0];
}

export function childPath(parent: ParentNode | null | undefined, ...localNames: string[]): Element | undefined {
  let current = parent;
  for (const name of localNames) {
    if (!current) return undefined;
    current = directChild(current, name);
  }
  return current instanceof Element ? current : undefined;
}

export function resolvePartTarget(sourcePart: string, target: string): string {
  if (/^[a-z][a-z0-9+.-]*:/i.test(target)) return target;
  const segments = target.startsWith('/') ? [] : sourcePart.split('/').slice(0, -1);
  for (const segment of target.replace(/^\/+/, '').split('/')) {
    if (!segment || segment === '.') continue;
    if (segment === '..') segments.pop();
    else segments.push(segment);
  }
  return segments.join('/');
}

export function contentTypeForPart(partPath: string): string {
  const extension = partPath.split('.').pop()?.toLowerCase();
  const types: Record<string, string> = {
    apng: 'image/apng',
    bmp: 'image/bmp',
    emf: 'image/emf',
    gif: 'image/gif',
    jpeg: 'image/jpeg',
    jpg: 'image/jpeg',
    png: 'image/png',
    svg: 'image/svg+xml',
    tif: 'image/tiff',
    tiff: 'image/tiff',
    webp: 'image/webp',
    wmf: 'image/wmf',
  };
  return types[extension ?? ''] ?? 'application/octet-stream';
}

export function bytesToDataUrl(bytes: Uint8Array, contentType: string): string {
  let binary = '';
  const chunkSize = 32_768;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return `data:${contentType};base64,${btoa(binary)}`;
}

function relationshipsPartPath(sourcePart: string): string {
  const separator = sourcePart.lastIndexOf('/');
  const directory = separator >= 0 ? sourcePart.slice(0, separator + 1) : '';
  const fileName = separator >= 0 ? sourcePart.slice(separator + 1) : sourcePart;
  return `${directory}_rels/${fileName}.rels`;
}
