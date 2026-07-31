import type { ArchitectureProject } from "./architecture-types";
import { operationalArchitectureProjects } from "./interfaces-operations";
import { serviceArchitectureProjects } from "./interfaces-services";
import { uiSecurityArchitectureProjects } from "./interfaces-ui-security";

export const interfaceArchitectureProjects: readonly ArchitectureProject[] = [
  ...serviceArchitectureProjects,
  ...uiSecurityArchitectureProjects,
  ...operationalArchitectureProjects,
];
