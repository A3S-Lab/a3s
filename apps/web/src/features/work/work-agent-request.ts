import type { TaskActions } from '../tasks/task-actions';
import { appState, appendTaskInstruction } from '../../state/app-state';
import { localPathInside, relativeLocalPath, sameLocalPath } from './work-local-files';
import type { WorkAgentProposalRequest } from './work-agent-proposal';

export interface WorkAgentRequest {
  workspaceRoot: string;
  paths: string[];
  instruction: string;
  selection?: string;
}

export interface WorkEditorAgentRequest extends Pick<WorkAgentRequest, 'instruction' | 'selection'> {
  proposal?: WorkAgentProposalRequest;
}

const MAX_SELECTION_LENGTH = 12_000;

export async function bindWorkAgentWorkspace(actions: TaskActions, workspaceRoot: string): Promise<void> {
  const root = workspaceRoot.trim();
  if (!root) throw new Error('请先选择本地工作文件夹。');
  const active = appState.sessions.find((session) => session.sessionId === appState.activeSessionId);
  if (active && (active.agentId !== 'work' || !sameLocalPath(active.workspace, root))) actions.newConversation();

  const compatible = appState.sessions.find((session) => session.sessionId === appState.activeSessionId);
  if (compatible?.agentId === 'work' && sameLocalPath(compatible.workspace, root)) {
    appState.workspaceRoot = root;
    return;
  }

  if (!appState.filesByDirectory[root]) await actions.selectNewTaskWorkspace(root);
  else appState.workspaceRoot = root;
}

export async function prepareWorkAgentRequest(actions: TaskActions, request: WorkAgentRequest): Promise<void> {
  await bindWorkAgentWorkspace(actions, request.workspaceRoot);
  const contextPaths = workAgentContextPaths(request.paths, request.workspaceRoot);
  appState.composerContextFiles = [...new Set([...appState.composerContextFiles, ...contextPaths])];
  appendTaskInstruction(workAgentInstruction(request));
}

export function workAgentContextPaths(paths: readonly string[], workspaceRoot: string): string[] {
  return [
    ...new Set(
      paths
        .filter((path) => localPathInside(workspaceRoot, path))
        .map((path) => relativeLocalPath(path, workspaceRoot))
        .filter(Boolean)
    ),
  ];
}

export function workAgentInstruction(request: Pick<WorkAgentRequest, 'instruction' | 'selection'>): string {
  const instruction = request.instruction.trim();
  const selection = request.selection?.trim();
  if (!selection) return instruction;
  const clipped = selection.slice(0, MAX_SELECTION_LENGTH);
  const suffix = selection.length > clipped.length ? '\n[选中内容已截断]' : '';
  return `${instruction}\n\n[选中内容]\n${clipped}${suffix}\n[/选中内容]`;
}
