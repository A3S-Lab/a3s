import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TaskActions } from '../tasks/task-actions';
import { appState } from '../../state/app-state';
import { bindWorkAgentWorkspace, prepareWorkAgentRequest, workAgentInstruction } from './work-agent-request';

const session = (sessionId: string, workspace: string, agentId = 'work') => ({
  sessionId,
  workspace,
  cwd: workspace,
  model: 'codex/gpt',
  followDefaultModel: false,
  permissionMode: 'default',
  state: 'connected',
  title: 'Task',
  agentId,
  createdAt: 1,
});

describe('Work Copilot request preparation', () => {
  beforeEach(() => {
    appState.sessions = [];
    appState.activeSessionId = null;
    appState.workspaceRoot = '';
    appState.filesByDirectory = {};
    appState.composerValue = '';
    appState.composerContextFiles = [];
    appState.composerSkills = [];
    appState.newTaskConfig = {
      workspace: '',
      model: 'codex/gpt',
      effort: 'medium',
      permissionMode: 'default',
      goal: '',
    };
  });

  it('starts a new conversation when the active task belongs to another workspace', async () => {
    appState.sessions = [session('code-task', '/code', 'default')];
    appState.activeSessionId = 'code-task';
    const newConversation = vi.fn(() => {
      appState.activeSessionId = null;
    });
    const selectNewTaskWorkspace = vi.fn(async (workspace: string) => {
      appState.newTaskConfig.workspace = workspace;
      appState.workspaceRoot = workspace;
      appState.filesByDirectory[workspace] = [];
    });
    const actions = { newConversation, selectNewTaskWorkspace } as unknown as TaskActions;

    await prepareWorkAgentRequest(actions, {
      workspaceRoot: '/docs',
      paths: ['/docs/Reports/Q2.docx', '/outside/private.txt'],
      instruction: '请总结文件。',
      selection: '重点段落',
    });

    expect(newConversation).toHaveBeenCalledTimes(1);
    expect(selectNewTaskWorkspace).toHaveBeenCalledWith('/docs');
    expect(appState.composerContextFiles).toEqual(['Reports/Q2.docx']);
    expect(appState.composerValue).toContain('请总结文件。');
    expect(appState.composerValue).toContain('[选中内容]\n重点段落\n[/选中内容]');
  });

  it('keeps a compatible conversation and its existing draft context', async () => {
    appState.sessions = [session('work-task', '/docs')];
    appState.activeSessionId = 'work-task';
    appState.filesByDirectory['/docs'] = [];
    appState.composerValue = '已有问题';
    appState.composerContextFiles = ['Overview.md'];
    const actions = {
      newConversation: vi.fn(),
      selectNewTaskWorkspace: vi.fn(),
    } as unknown as TaskActions;

    await bindWorkAgentWorkspace(actions, '/docs');
    await prepareWorkAgentRequest(actions, {
      workspaceRoot: '/docs',
      paths: ['/docs/Overview.md', '/docs/Budget.xlsx'],
      instruction: '比较两份材料。',
    });

    expect(actions.newConversation).not.toHaveBeenCalled();
    expect(actions.selectNewTaskWorkspace).not.toHaveBeenCalled();
    expect(appState.composerContextFiles).toEqual(['Overview.md', 'Budget.xlsx']);
    expect(appState.composerValue).toBe('已有问题\n\n比较两份材料。');
  });

  it('bounds very large selected text before adding it to the draft', () => {
    const prompt = workAgentInstruction({ instruction: '改写', selection: 'a'.repeat(12_100) });
    expect(prompt).toContain('[选中内容已截断]');
    expect(prompt.length).toBeLessThan(12_100);
  });
});
