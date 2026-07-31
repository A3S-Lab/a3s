import {
  architectureProjects,
  type ArchitectureCategory,
  type ArchitectureProject,
} from ".";

export type ArchitectureAtlasCategory = "all" | ArchitectureCategory;

export function filterArchitectureProjects(
  category: ArchitectureAtlasCategory,
  query: string,
): readonly ArchitectureProject[] {
  const projects: readonly ArchitectureProject[] =
    category === "all"
      ? architectureProjects
      : architectureProjects.filter((project) => project.category === category);
  const normalizedQuery = query.trim().toLocaleLowerCase();

  if (!normalizedQuery) return projects;

  return projects.filter((project) => {
    const nodes = project.nodes
      .map((node) => `${node.label} ${node.detail.cn} ${node.detail.en}`)
      .join(" ");
    const searchable =
      `${project.name} ${project.id} ${project.role.cn} ${project.role.en} ${nodes}`.toLocaleLowerCase();

    return searchable.includes(normalizedQuery);
  });
}

export function replacementArchitectureProject(
  projects: readonly ArchitectureProject[],
  activeProjectId: string,
  query = "",
): ArchitectureProject | undefined {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const exactMatch = normalizedQuery
    ? projects.find(
        (project) =>
          project.name.toLocaleLowerCase() === normalizedQuery ||
          project.id.toLocaleLowerCase() === normalizedQuery,
      )
    : undefined;

  if (exactMatch && exactMatch.id !== activeProjectId) return exactMatch;
  if (projects.some((project) => project.id === activeProjectId))
    return undefined;
  return projects[0];
}
