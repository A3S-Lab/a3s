export type ArchitectureCategory = "products" | "runtime" | "interfaces";

export type ArchitectureNodeKind =
  | "entry"
  | "process"
  | "control"
  | "service"
  | "adapter"
  | "runtime"
  | "store"
  | "security"
  | "output";

export type ArchitectureEdgeKind =
  "call" | "data" | "event" | "control" | "optional";

export interface LocalizedArchitectureText {
  cn: string;
  en: string;
}

export interface ArchitecturePoint {
  x: number;
  y: number;
}

export interface ArchitectureNode {
  id: string;
  label: string;
  detail: LocalizedArchitectureText;
  kind: ArchitectureNodeKind;
  position: ArchitecturePoint;
}

export interface ArchitectureEdge {
  from: string;
  to: string;
  label: LocalizedArchitectureText;
  kind: ArchitectureEdgeKind;
  bidirectional?: boolean;
}

export interface ArchitectureGroup {
  id: string;
  label: LocalizedArchitectureText;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface ArchitectureEvidence {
  label: string;
  href: string;
}

export interface ArchitectureProject {
  id: string;
  name: string;
  category: ArchitectureCategory;
  role: LocalizedArchitectureText;
  href: string;
  nodes: readonly ArchitectureNode[];
  edges: readonly ArchitectureEdge[];
  groups?: readonly ArchitectureGroup[];
  evidence: readonly ArchitectureEvidence[];
}

export function localized(cn: string, en: string): LocalizedArchitectureText {
  return { cn, en };
}

export function architectureNode(
  id: string,
  label: string,
  kind: ArchitectureNodeKind,
  x: number,
  y: number,
  cn: string,
  en: string,
): ArchitectureNode {
  return { id, label, kind, position: { x, y }, detail: localized(cn, en) };
}

export function architectureEdge(
  from: string,
  to: string,
  kind: ArchitectureEdgeKind,
  cn: string,
  en: string,
  bidirectional = false,
): ArchitectureEdge {
  return { from, to, kind, label: localized(cn, en), bidirectional };
}

export function architectureGroup(
  id: string,
  cn: string,
  en: string,
  x: number,
  y: number,
  width: number,
  height: number,
): ArchitectureGroup {
  return { id, label: localized(cn, en), x, y, width, height };
}
