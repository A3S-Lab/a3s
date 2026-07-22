import { useCallback, useEffect, useRef, useState } from 'react';
import type { WorkPresentationContent } from '../work-types';

const PRESENTATION_HISTORY_LIMIT = 100;

interface PresentationHistoryState {
  past: WorkPresentationContent[];
  present: WorkPresentationContent;
  future: WorkPresentationContent[];
}

export function usePresentationHistory({
  content,
  onChange,
}: {
  content: WorkPresentationContent;
  onChange: (content: WorkPresentationContent) => void;
}) {
  const historyRef = useRef<PresentationHistoryState>({ past: [], present: content, future: [] });
  const applyingHistoryRef = useRef(false);
  const onChangeRef = useRef(onChange);
  const [, setVersion] = useState(0);
  onChangeRef.current = onChange;

  useEffect(() => {
    const history = historyRef.current;
    if (applyingHistoryRef.current) {
      applyingHistoryRef.current = false;
      history.present = content;
      setVersion((value) => value + 1);
      return;
    }
    if (content === history.present) return;
    history.past = [...history.past.slice(-(PRESENTATION_HISTORY_LIMIT - 1)), history.present];
    history.present = content;
    history.future = [];
    setVersion((value) => value + 1);
  }, [content]);

  const undo = useCallback((): boolean => {
    const history = historyRef.current;
    const previous = history.past.at(-1);
    if (!previous) return false;
    history.past = history.past.slice(0, -1);
    history.future = [...history.future, history.present];
    history.present = previous;
    applyingHistoryRef.current = true;
    setVersion((value) => value + 1);
    onChangeRef.current(previous);
    return true;
  }, []);

  const redo = useCallback((): boolean => {
    const history = historyRef.current;
    const next = history.future.at(-1);
    if (!next) return false;
    history.future = history.future.slice(0, -1);
    history.past = [...history.past, history.present].slice(-PRESENTATION_HISTORY_LIMIT);
    history.present = next;
    applyingHistoryRef.current = true;
    setVersion((value) => value + 1);
    onChangeRef.current(next);
    return true;
  }, []);

  return {
    canUndo: historyRef.current.past.length > 0,
    canRedo: historyRef.current.future.length > 0,
    undo,
    redo,
  };
}
