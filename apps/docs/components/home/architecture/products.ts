import type { ArchitectureProject } from './architecture-types';
import { productApplicationArchitectures } from './products-applications';
import { productCapabilityArchitectures } from './products-capabilities';
import { productContentArchitectures } from './products-content';
import { productExecutionArchitectures } from './products-execution';

export const productArchitectureProjects: readonly ArchitectureProject[] = [
  ...productApplicationArchitectures,
  ...productExecutionArchitectures,
  ...productCapabilityArchitectures,
  ...productContentArchitectures,
];
