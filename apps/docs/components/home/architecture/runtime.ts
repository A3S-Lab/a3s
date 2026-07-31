import type { ArchitectureProject } from "./architecture-types";
import { coordinationArchitectureProjects } from "./runtime-coordination";
import { dataArchitectureProjects } from "./runtime-data";

export const runtimeArchitectureProjects: readonly ArchitectureProject[] = [
  ...coordinationArchitectureProjects,
  ...dataArchitectureProjects,
];
