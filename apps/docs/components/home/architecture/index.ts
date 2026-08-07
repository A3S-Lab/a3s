import type { ArchitectureProject } from './architecture-types';
import { interfaceArchitectureProjects } from './interfaces';
import { productArchitectureProjects } from './products';
import { runtimeArchitectureProjects } from './runtime';

export type {
  ArchitectureCategory,
  ArchitectureNode,
  ArchitectureProject,
  ArchitectureTone,
  LocalizedArchitectureText,
} from './architecture-types';

export const architectureProjects: readonly ArchitectureProject[] = [
  ...productArchitectureProjects,
  ...runtimeArchitectureProjects,
  ...interfaceArchitectureProjects,
];
