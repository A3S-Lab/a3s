export type ArchitectureCategory = 'products' | 'runtime' | 'interfaces';

export type ArchitectureTone = 'surface' | 'core' | 'contract' | 'runtime' | 'evidence';

export interface LocalizedArchitectureText {
  cn: string;
  en: string;
}

export interface ArchitectureNode {
  id: string;
  label: string;
  detail: LocalizedArchitectureText;
  tone: ArchitectureTone;
}

export interface ArchitectureProject {
  id: string;
  name: string;
  category: ArchitectureCategory;
  role: LocalizedArchitectureText;
  href: string;
  nodes: readonly ArchitectureNode[];
  links?: ReadonlyArray<readonly [string, string]>;
}

export function localized(cn: string, en: string): LocalizedArchitectureText {
  return { cn, en };
}

export function architectureNode(
  id: string,
  label: string,
  tone: ArchitectureTone,
  cn: string,
  en: string,
): ArchitectureNode {
  return { id, label, tone, detail: localized(cn, en) };
}
