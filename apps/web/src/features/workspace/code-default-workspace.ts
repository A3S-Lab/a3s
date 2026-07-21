export interface CodeDefaultWorkspaceSource {
  newTaskWorkspace?: string | null;
  serviceWorkspace?: string | null;
  currentWorkspace?: string | null;
}

export function codeDefaultWorkspace(source: CodeDefaultWorkspaceSource): string {
  return source.newTaskWorkspace?.trim() || source.serviceWorkspace?.trim() || source.currentWorkspace?.trim() || '';
}
