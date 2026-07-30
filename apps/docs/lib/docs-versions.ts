import { localePath, stripLocalePrefix, type Locale } from '@/lib/i18n';

export const latestDocsVersion = 'latest';

export interface DocsVersion {
  id: string;
  topics: string[];
}

export interface DocsProductVersions {
  product: string;
  versions: DocsVersion[];
}

export interface ActiveDocsVersion {
  product: DocsProductVersions;
  version: DocsVersion;
  topic: string;
}

const versionSegment = /^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

function compareVersions(left: DocsVersion, right: DocsVersion): number {
  if (left.id === latestDocsVersion) return -1;
  if (right.id === latestDocsVersion) return 1;
  return right.id.localeCompare(left.id, 'en', { numeric: true, sensitivity: 'base' });
}

/**
 * Build the version catalog from the localized content tree. A directory named
 * `vX.Y.Z` below a product is an immutable version; all other pages belong to
 * the continuously updated `latest` documentation.
 */
export function buildDocsVersionCatalog(
  pageSlugs: ReadonlyArray<ReadonlyArray<string>>,
): DocsProductVersions[] {
  const products = new Map<string, Map<string, Set<string>>>();

  for (const slugs of pageSlugs) {
    const [product, possibleVersion] = slugs;
    if (!product) continue;

    const version = possibleVersion && versionSegment.test(possibleVersion)
      ? possibleVersion
      : latestDocsVersion;
    const topicStart = version === latestDocsVersion ? 1 : 2;
    const topic = slugs.slice(topicStart).join('/');

    const versions = products.get(product) ?? new Map<string, Set<string>>();
    const topics = versions.get(version) ?? new Set<string>();
    topics.add(topic);
    versions.set(version, topics);
    products.set(product, versions);
  }

  return [...products]
    .map(([product, versions]) => ({
      product,
      versions: [...versions]
        .map(([id, topics]) => ({ id, topics: [...topics].sort() }))
        .sort(compareVersions),
    }))
    .filter(({ versions }) => versions.some(({ id }) => id !== latestDocsVersion))
    .sort((left, right) => left.product.localeCompare(right.product));
}

export function matchDocsVersion(
  pathname: string,
  catalog: ReadonlyArray<DocsProductVersions>,
): ActiveDocsVersion | null {
  const cleanPath = stripLocalePrefix(pathname.split(/[?#]/, 1)[0]);
  const segments = cleanPath.split('/').filter(Boolean);
  if (segments[0] !== 'docs' || !segments[1]) return null;

  const product = catalog.find((candidate) => candidate.product === segments[1]);
  if (!product) return null;

  const explicitVersion = product.versions.find(({ id }) => id === segments[2]);
  const version = explicitVersion
    ?? product.versions.find(({ id }) => id === latestDocsVersion);
  if (!version) return null;

  return {
    product,
    version,
    topic: segments.slice(explicitVersion ? 3 : 2).join('/'),
  };
}

export function docsVersionPath(
  pathname: string,
  targetVersion: string,
  catalog: ReadonlyArray<DocsProductVersions>,
  locale: Locale,
): string | null {
  const active = matchDocsVersion(pathname, catalog);
  if (!active) return null;

  const target = active.product.versions.find(({ id }) => id === targetVersion);
  if (!target) return null;

  const segments = ['docs', active.product.product];
  if (target.id !== latestDocsVersion) segments.push(target.id);
  if (active.topic && target.topics.includes(active.topic)) segments.push(active.topic);

  return localePath(`/${segments.join('/')}`, locale);
}
