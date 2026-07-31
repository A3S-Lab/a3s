import { interfaceArchitectureProjects } from "./interfaces";
import { productArchitectureProjects } from "./products";
import { runtimeArchitectureProjects } from "./runtime";

export type {
  ArchitectureCategory,
  ArchitectureEdge,
  ArchitectureEdgeKind,
  ArchitectureEvidence,
  ArchitectureGroup,
  ArchitectureNode,
  ArchitectureNodeKind,
  ArchitecturePoint,
  ArchitectureProject,
  LocalizedArchitectureText,
} from "./architecture-types";

export const architectureProjects = [
  ...productArchitectureProjects,
  ...runtimeArchitectureProjects,
  ...interfaceArchitectureProjects,
] as const;

export const architectureProjectCount = architectureProjects.length;
export const architectureNodeCount = architectureProjects.reduce(
  (count, project) => count + project.nodes.length,
  0,
);
