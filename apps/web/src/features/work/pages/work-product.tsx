import { useCallback, useEffect, useRef, useState } from 'react';
import { useSnapshot } from 'valtio';
import { codeApi } from '../../../lib/api';
import {
  appState,
  formatApiError,
  navigateConversation,
  navigateWorkHome,
  sessionTitle,
  showToast,
} from '../../../state/app-state';
import type { CodeSession, WorkspaceEntry, WorkspaceFileCatalogItem } from '../../../types/api';
import type { CodeActions } from '../../code/use-code-controller';
import { parseGoalCommand } from '../../tasks/goal-command';
import { TaskLibrary } from '../../tasks/components/task-library';
import { codeDefaultWorkspace } from '../../workspace/code-default-workspace';
import { WorkspaceQuickOpen } from '../../workspace/components/workspace-quick-open';
import { WorkCodeWorkspace } from '../components/work-code-workspace';
import { WorkCompatibilityDialog } from '../components/work-compatibility-dialog';
import { WorkConversation } from '../components/work-conversation';
import { readWorkCopilotWidth, WorkCopilot } from '../components/work-copilot';
import { WorkEditorShell } from '../components/work-editor-shell';
import { WorkFilesWorkspace } from '../components/work-files-workspace';
import { WorkHome } from '../components/work-home';
import { WorkLivePreviewPanel } from '../components/work-live-preview-panel';
import { isOfficeShortcutBlocked } from '../editors/office-shortcuts';
import { useWorkCodeController } from '../use-work-code-controller';
import { useWorkController } from '../use-work-controller';
import { useWorkFilesController } from '../use-work-files-controller';
import { useOfficeAutomationStatus } from '../use-office-automation-status';
import { type WorkAgentProposalRequest, workAgentProposalInstruction } from '../work-agent-proposal';
import { prepareWorkAgentRequest, type WorkAgentRequest, type WorkEditorAgentRequest } from '../work-agent-request';
import { WORK_IMPORT_ACCEPT } from '../work-file-io';
import { isWorkOfficePath, isWorkTextEditorEntry, localPathBasename, workFileMimeType } from '../work-local-files';
import { workOfficeAgentInstruction } from '../work-office-agent';

const surfaceStorageKey = 'a3s-work.surface';
const copilotStorageKey = 'a3s-work.copilot-open';
const previewWidthStorageKey = 'a3s-work.preview-width';

export function WorkProduct({ actions: codeActions }: { actions: CodeActions }) {
  const state = useSnapshot(appState);
  const actions = useWorkController();
  const files = useWorkFilesController(
    codeDefaultWorkspace({
      newTaskWorkspace: state.newTaskConfig.workspace,
      serviceWorkspace: state.health?.workspace,
      currentWorkspace: state.workspaceRoot,
    })
  );
  const code = useWorkCodeController(files.rootPath);
  const officeAutomation = useOfficeAutomationStatus();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [surface, setSurface] = useState<'files' | 'library'>(readSurface);
  const [copilotOpen, setCopilotOpen] = useState(readCopilotOpen);
  const [copilotWidth, setCopilotWidth] = useState(readWorkCopilotWidth);
  const [previewTarget, setPreviewTarget] = useState(readInitialPreviewTarget);
  const [previewWidth, setPreviewWidth] = useState(readPreviewWidth);
  const [openingPath, setOpeningPath] = useState<string | null>(null);
  const [agentProposal, setAgentProposal] = useState<WorkAgentProposalRequest | null>(null);
  const [localCreateRequest, setLocalCreateRequest] = useState<{
    requestId: number;
    templateId: string;
    directory: string;
  } | null>(null);
  const localCreateRequestIdRef = useRef(0);
  const pendingHomeSubmissionRef = useRef(false);
  const homeSubmissionStartedRef = useRef(false);
  const previousArtifactIdRef = useRef(actions.activeArtifact?.id ?? null);
  const openFilePicker = () => fileInputRef.current?.click();
  const updateSurface = (next: 'files' | 'library') => {
    setSurface(next);
    persistValue(surfaceStorageKey, next);
  };
  const updateCopilotOpen = useCallback((open: boolean) => {
    setCopilotOpen(open);
    persistValue(copilotStorageKey, String(open));
  }, []);
  const updatePreviewTarget = useCallback((target: string | null) => {
    setPreviewTarget(target);
    try {
      const url = new URL(window.location.href);
      if (target) url.searchParams.set('preview', target);
      else url.searchParams.delete('preview');
      window.history.replaceState(window.history.state, '', `${url.pathname}${url.search}${url.hash}`);
    } catch {
      // The panel remains usable when URL history is unavailable.
    }
  }, []);
  const requestLocalArtifactCreate = useCallback(
    async (templateId: string) => {
      let directory = files.currentPath || files.rootPath;
      if (!files.rootPath) directory = (await files.pickRoot()) ?? '';
      if (!directory) return;
      localCreateRequestIdRef.current += 1;
      setLocalCreateRequest({
        requestId: localCreateRequestIdRef.current,
        templateId,
        directory,
      });
    },
    [files.currentPath, files.pickRoot, files.rootPath]
  );
  const createForSurface = useCallback(
    (templateId: string) => {
      if (surface === 'files') {
        void requestLocalArtifactCreate(templateId);
        return;
      }
      void actions.createArtifact(templateId);
    },
    [actions.createArtifact, requestLocalArtifactCreate, surface]
  );
  const findQuickOpenFiles = useCallback(
    (query: string, maxResults: number) => codeApi.workspaceFiles(files.rootPath, query, maxResults),
    [files.rootPath]
  );

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        event.defaultPrevented ||
        event.repeat ||
        event.altKey ||
        event.shiftKey ||
        isOfficeShortcutBlocked(event.target) ||
        !(event.metaKey || event.ctrlKey)
      ) {
        return;
      }
      const key = event.key.toLocaleLowerCase();
      if (key === 'n') {
        event.preventDefault();
        createForSurface('blank-document');
      } else if (key === 'o') {
        event.preventDefault();
        void files.pickRoot().then((path) => {
          if (path) updateSurface('files');
        });
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [createForSurface, files.pickRoot]);
  useEffect(() => {
    const artifactId = actions.activeArtifact?.id ?? null;
    if (previousArtifactIdRef.current !== artifactId) {
      previousArtifactIdRef.current = artifactId;
      setAgentProposal(null);
    }
  }, [actions.activeArtifact?.id]);
  useEffect(() => {
    if (!pendingHomeSubmissionRef.current) return;
    if (state.taskSubmissionState) homeSubmissionStartedRef.current = true;
    if (state.activeSessionId) {
      pendingHomeSubmissionRef.current = false;
      homeSubmissionStartedRef.current = false;
      updateCopilotOpen(false);
      updatePreviewTarget(null);
      navigateConversation(state.activeSessionId);
      return;
    }
    if (homeSubmissionStartedRef.current && !state.taskSubmissionState) {
      pendingHomeSubmissionRef.current = false;
      homeSubmissionStartedRef.current = false;
    }
  }, [state.activeSessionId, state.taskSubmissionState, updateCopilotOpen, updatePreviewTarget]);
  const openLocalItem = async (
    entry: Pick<WorkspaceEntry, 'path' | 'name' | 'isBinary' | 'isDirectory'>
  ): Promise<boolean> => {
    if (openingPath) return false;
    setOpeningPath(entry.path);
    try {
      if (isWorkTextEditorEntry(entry)) {
        return await code.openFile({ path: entry.path, isBinary: false });
      }
      if (!isWorkOfficePath(entry.path)) {
        showToast('这个文件暂不能直接编辑。', 'info');
        return false;
      }
      const bytes = await codeApi.readBinaryFile(entry.path);
      const data = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
      const file = new File([data], entry.name, { type: workFileMimeType(entry.path) });
      await actions.importFile(file, { localPath: entry.path });
      return true;
    } catch (error) {
      showToast(formatApiError(error), 'error');
      return false;
    } finally {
      setOpeningPath(null);
    }
  };
  const openLocalFile = async (entry: WorkspaceEntry) => {
    await openLocalItem(entry);
  };
  const openQuickFile = (entry: WorkspaceFileCatalogItem) =>
    openLocalItem({
      path: entry.path,
      name: entry.name,
      isBinary: entry.isBinary,
      isDirectory: false,
    });
  const workTaskActions: CodeActions = {
    ...codeActions,
    selectFile: async (selection) => {
      const opened = await openLocalItem({
        path: selection.path,
        name: localPathBasename(selection.path),
        isBinary: selection.isBinary,
        isDirectory: false,
      });
      if (opened) {
        updateCopilotOpen(false);
        navigateWorkHome({ history: 'push' });
      }
      return opened;
    },
  };
  const requestAgent = async (request: WorkAgentRequest, proposal?: WorkAgentProposalRequest) => {
    let workspaceRoot = request.workspaceRoot || files.rootPath;
    if (!workspaceRoot) workspaceRoot = (await files.pickRoot()) ?? '';
    if (!workspaceRoot) return;
    updateCopilotOpen(true);
    try {
      await prepareWorkAgentRequest(codeActions, {
        ...request,
        workspaceRoot,
      });
      if (proposal) setAgentProposal(proposal);
    } catch (error) {
      showToast(formatApiError(error), 'error');
    }
  };
  const requestDocumentAgent = (request: WorkEditorAgentRequest) => {
    const title = actions.activeArtifact?.title || '当前文档';
    const proposal = request.proposal?.targets.length ? request.proposal : undefined;
    const localPath = actions.activeLocalBinding?.path;
    const instruction = proposal
      ? workAgentProposalInstruction(request.instruction, proposal)
      : workOfficeAgentInstruction({ title, instruction: request.instruction, localPath });
    return requestAgent(
      {
        workspaceRoot: files.rootPath,
        paths: localPath ? [localPath] : [],
        instruction,
        selection: request.selection,
      },
      proposal
    );
  };
  const startNewConversation = () => {
    pendingHomeSubmissionRef.current = false;
    homeSubmissionStartedRef.current = false;
    codeActions.newConversation();
    updateCopilotOpen(false);
    setAgentProposal(null);
    updateSurface('library');
    actions.setLibraryView('home');
    code.closeWorkspace();
    updatePreviewTarget(null);
    void actions.closeArtifact();
    navigateWorkHome({ history: 'replace' });
  };
  const selectConversation = async (session: CodeSession) => {
    const selection = codeActions.selectSession(session.sessionId);
    updateCopilotOpen(false);
    updatePreviewTarget(null);
    navigateConversation(session.sessionId);
    await selection;
    if (session.workspace) await files.selectRoot(session.workspace);
  };
  const openHome = async () => {
    updateCopilotOpen(false);
    updatePreviewTarget(null);
    code.closeWorkspace();
    if (actions.activeArtifact) await actions.closeArtifact();
    updateSurface('library');
    actions.setLibraryView('home');
    navigateWorkHome({ history: 'replace' });
  };
  const openWorkspaceFromConversation = async () => {
    updateCopilotOpen(false);
    updatePreviewTarget(null);
    code.closeWorkspace();
    if (actions.activeArtifact) await actions.closeArtifact();
    updateSurface('files');
    if (files.rootPath) files.navigateTo(files.rootPath);
    navigateWorkHome({ history: 'push' });
  };
  const openActiveConversation = () => {
    if (!state.activeSessionId) return;
    updateCopilotOpen(false);
    updatePreviewTarget(null);
    navigateConversation(state.activeSessionId);
  };
  const submitHomeTask = (content: string) => {
    if (parseGoalCommand(content)) return;
    updateCopilotOpen(false);
    updatePreviewTarget(null);
    if (appState.activeSessionId) {
      navigateConversation(appState.activeSessionId);
      return;
    }
    pendingHomeSubmissionRef.current = true;
    homeSubmissionStartedRef.current = false;
  };
  const activeSession = state.activeSessionId
    ? state.sessions.find((session) => session.sessionId === state.activeSessionId)
    : undefined;

  return (
    <section className='work-product'>
      <input
        ref={fileInputRef}
        className='work-file-input'
        type='file'
        accept={WORK_IMPORT_ACCEPT}
        aria-label='打开 Office 文件'
        onChange={(event) => {
          const file = event.target.files?.[0];
          event.target.value = '';
          if (file) void actions.importFile(file);
        }}
      />
      {actions.pendingImport?.artifact.compatibility && (
        <WorkCompatibilityDialog
          report={actions.pendingImport.artifact.compatibility}
          mode='import'
          busy={actions.importing}
          onClose={actions.cancelImport}
          onConfirm={() => void actions.confirmImport()}
        />
      )}
      {state.sidebarOpen && (
        <TaskLibrary
          actions={codeActions}
          onNewConversation={startNewConversation}
          onSelectSession={selectConversation}
        />
      )}
      {copilotOpen &&
        state.workRoute !== 'conversation' &&
        (actions.activeArtifact || code.tabs.length || surface === 'files') && (
          <WorkCopilot
            actions={workTaskActions}
            workspaceRoot={files.rootPath}
            currentPath={actions.activeLocalBinding?.path || code.activePath || files.currentPath}
            onClose={() => updateCopilotOpen(false)}
            onPickRoot={async () => {
              await files.pickRoot();
            }}
            onAgentRequest={requestAgent}
            width={copilotWidth}
            onWidthChange={setCopilotWidth}
            proposal={agentProposal}
            onDismissProposal={() => setAgentProposal(null)}
            onNewConversation={startNewConversation}
            workspaceMode={actions.activeArtifact ? 'office' : code.tabs.length ? 'code' : 'files'}
            officeAutomation={officeAutomation}
          />
        )}
      <div className='work-primary-pane'>
        {state.workRoute === 'conversation' ? (
          <WorkConversation
            actions={workTaskActions}
            sessionId={state.conversationSessionId}
            sidebarOpen={state.sidebarOpen}
            onOpenSidebar={() => {
              appState.sidebarOpen = true;
            }}
            onHome={() => {
              void openHome();
            }}
            onNewTask={startNewConversation}
            onOpenWorkspace={() => {
              void openWorkspaceFromConversation();
            }}
          />
        ) : actions.activeArtifact ? (
          <WorkEditorShell
            actions={actions}
            copilotOpen={copilotOpen}
            onToggleCopilot={() => updateCopilotOpen(!copilotOpen)}
            onAgentRequest={requestDocumentAgent}
            defaultLocalDirectory={files.currentPath || files.rootPath}
            onPickLocalDirectory={async () => {
              const selection = await codeApi.pickWorkspaceDirectory(files.currentPath || files.rootPath || undefined);
              return selection.cancelled ? null : selection.path;
            }}
            onLocalFileSaved={() => void files.refresh()}
          />
        ) : code.tabs.length ? (
          <WorkCodeWorkspace
            actions={code}
            rootPath={files.rootPath}
            assistantOpen={copilotOpen}
            onOpenEntry={openLocalFile}
            onBack={() => {
              code.closeWorkspace();
            }}
            onToggleAssistant={() => updateCopilotOpen(!copilotOpen)}
            onAgentRequest={requestAgent}
            previewTarget={previewTarget}
            onPreviewTarget={(target) => updatePreviewTarget(target)}
          />
        ) : (
          <>
            {surface === 'files' ? (
              <WorkFilesWorkspace
                actions={files}
                openingPath={openingPath}
                copilotOpen={copilotOpen}
                sidebarOpen={state.sidebarOpen}
                onOpenFile={openLocalFile}
                onAgentRequest={requestAgent}
                onCreateArtifact={(templateId) => void requestLocalArtifactCreate(templateId)}
                createArtifactRequest={localCreateRequest}
                onCreateArtifactFile={async (request, fileName) => {
                  const result = await actions.createLocalArtifact(request.templateId, request.directory, fileName);
                  if (result === 'created') await files.refresh();
                  return result;
                }}
                onConsumeCreateArtifactRequest={() => setLocalCreateRequest(null)}
                onOpenSidebar={() => {
                  appState.sidebarOpen = true;
                }}
                onOpenHome={() => {
                  updateSurface('library');
                  actions.setLibraryView('home');
                }}
                onToggleCopilot={() => updateCopilotOpen(!copilotOpen)}
                onPreviewEntry={(entry) => updatePreviewTarget(entry.path)}
              />
            ) : (
              <WorkHome
                artifacts={actions.artifacts}
                folders={actions.folders}
                view={actions.libraryView}
                activeFolderId={actions.activeFolderId}
                loading={actions.loading}
                error={actions.loadError}
                sidebarOpen={state.sidebarOpen}
                taskActions={codeActions}
                activeSessionTitle={activeSession ? sessionTitle(activeSession, state.sessionTitles) : null}
                onOpenSidebar={() => {
                  appState.sidebarOpen = true;
                }}
                onContinueSession={openActiveConversation}
                onNewTask={startNewConversation}
                onTaskSubmit={submitHomeTask}
                onOpenWorkspace={() => {
                  updateSurface('files');
                  if (files.rootPath) files.navigateTo(files.rootPath);
                }}
                onCreate={(templateId) => void actions.createArtifact(templateId)}
                onOpen={(id) => void actions.openArtifact(id)}
                onImport={openFilePicker}
                onChangeView={(view) => actions.setLibraryView(view)}
                onToggleFavorite={actions.toggleFavorite}
                onRename={(id, title) => actions.patchStoredArtifact(id, { title: title.trim() })}
                onCopy={(id) => void actions.copyArtifact(id)}
                onMove={(id, folderId) => void actions.patchStoredArtifact(id, { folderId })}
                onRestore={(id) => void actions.restoreArtifact(id)}
                onDelete={(artifact) => void actions.removeArtifact(artifact.id)}
                onOpenFolder={actions.openFolder}
                onCreateFolder={actions.createFolder}
                onRenameFolder={(id, name) => actions.patchFolder(id, { name: name.trim() })}
                onRestoreFolder={(id) => void actions.restoreFolder(id)}
                onDeleteFolder={(folder) => void actions.removeFolder(folder.id)}
                onRetry={() => void actions.refresh()}
              />
            )}
          </>
        )}
      </div>
      {state.workRoute !== 'conversation' && previewTarget && (
        <WorkLivePreviewPanel
          target={previewTarget}
          width={previewWidth}
          onWidthChange={(nextWidth) => {
            setPreviewWidth(nextWidth);
            persistValue(previewWidthStorageKey, String(Math.round(nextWidth)));
          }}
          onTargetChange={(target) => updatePreviewTarget(target)}
          onClose={() => updatePreviewTarget(null)}
        />
      )}
      {state.workRoute !== 'conversation' && state.fileQuickOpenOpen && (
        <WorkspaceQuickOpen
          rootPath={files.rootPath}
          generation={state.workspaceGeneration}
          openFiles={code.tabs.map((tab) => ({ path: tab.path, isBinary: false }))}
          activePath={code.activePath}
          findFiles={findQuickOpenFiles}
          openFile={openQuickFile}
          onClose={() => {
            appState.fileQuickOpenOpen = false;
          }}
        />
      )}
    </section>
  );
}

function readSurface(): 'files' | 'library' {
  try {
    return localStorage.getItem(surfaceStorageKey) === 'files' ? 'files' : 'library';
  } catch {
    return 'library';
  }
}

function readCopilotOpen(): boolean {
  try {
    return localStorage.getItem(copilotStorageKey) !== 'false';
  } catch {
    return true;
  }
}

function readInitialPreviewTarget(): string | null {
  try {
    return new URL(window.location.href).searchParams.get('preview');
  } catch {
    return null;
  }
}

function readPreviewWidth(): number {
  try {
    const width = Number(localStorage.getItem(previewWidthStorageKey));
    if (Number.isFinite(width) && width >= 380) return width;
  } catch {
    // Use the product default when browser storage is unavailable.
  }
  return 620;
}

function persistValue(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // The current product state remains usable when browser storage is unavailable.
  }
}
