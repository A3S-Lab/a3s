import { useEffect, useRef, useState } from 'react';
import { codeApi } from '../../lib/api';
import { formatApiError } from '../../state/app-state';
import { searchWorkLocalFiles, type WorkLocalFileSearchResult } from './work-local-file-search';

export type WorkFileSearchScope = 'folder' | 'workspace';

interface WorkFileSearchState extends WorkLocalFileSearchResult {
  loading: boolean;
  error: string | null;
}

const idleState: WorkFileSearchState = {
  entries: [],
  scannedDirectories: 0,
  scannedEntries: 0,
  unreadableDirectories: 0,
  truncated: false,
  loading: false,
  error: null,
};

export function useWorkFileSearch({
  rootPath,
  query,
  scope,
  filesystemRevision,
}: {
  rootPath: string;
  query: string;
  scope: WorkFileSearchScope;
  filesystemRevision: number;
}): WorkFileSearchState {
  const [state, setState] = useState<WorkFileSearchState>(idleState);
  const requestSequence = useRef(0);

  useEffect(() => {
    requestSequence.current += 1;
    const sequence = requestSequence.current;
    const normalizedQuery = query.trim();
    if (scope !== 'workspace' || !rootPath || !normalizedQuery) {
      setState(idleState);
      return;
    }

    setState({ ...idleState, loading: true });
    const timer = window.setTimeout(() => {
      void searchWorkLocalFiles(codeApi.readDir, rootPath, normalizedQuery)
        .then((result) => {
          if (sequence === requestSequence.current) setState({ ...result, loading: false, error: null });
        })
        .catch((error) => {
          if (sequence === requestSequence.current) {
            setState({ ...idleState, error: formatApiError(error) });
          }
        });
    }, 180);
    return () => window.clearTimeout(timer);
  }, [filesystemRevision, query, rootPath, scope]);

  return state;
}
