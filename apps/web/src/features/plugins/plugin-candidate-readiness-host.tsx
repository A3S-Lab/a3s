import { useCallback, useEffect, useRef, useState } from 'react';
import { ApiError, codeApi } from '../../lib/api';
import type { PluginUiCandidate, PluginUiCandidateDecision } from '../../types/api';
import { activityCandidateHostInit, parsePluginCandidateMessage } from './plugin-protocol';

const CANDIDATE_POLL_MS = 400;
const CANDIDATE_READY_TIMEOUT_MS = 10_000;
const DECISION_RETRY_DELAYS_MS = [0, 120, 360] as const;

/**
 * Invisible, authority-free browser preflight for the next UI generation.
 *
 * The candidate receives only a dedicated MessagePort and a readiness-mode
 * `host.init`. It is never connected to context, state, Tool, MCP, Flow, or
 * other backend bindings. A failed proof therefore leaves the currently
 * selected Registry generation untouched.
 */
export function PluginCandidateReadinessHost() {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const portRef = useRef<MessagePort | null>(null);
  const readyTimeoutRef = useRef<number | null>(null);
  const loadedTokenRef = useRef<string | null>(null);
  const loadCountRef = useRef(0);
  const decisionSentRef = useRef(false);
  const candidateRef = useRef<PluginUiCandidate | null>(null);
  const [candidate, setCandidate] = useState<PluginUiCandidate | null>(null);
  candidateRef.current = candidate;

  const closePort = useCallback(() => {
    if (readyTimeoutRef.current !== null) {
      window.clearTimeout(readyTimeoutRef.current);
      readyTimeoutRef.current = null;
    }
    const port = portRef.current;
    if (port) {
      port.onmessage = null;
      port.onmessageerror = null;
      port.close();
      portRef.current = null;
    }
  }, []);

  const reportDecision = useCallback(
    async (token: string, decision: PluginUiCandidateDecision) => {
      if (candidateRef.current?.token !== token || decisionSentRef.current) return;
      decisionSentRef.current = true;
      closePort();
      for (const delay of DECISION_RETRY_DELAYS_MS) {
        if (delay > 0) await wait(delay);
        if (candidateRef.current?.token !== token) return;
        try {
          await codeApi.decidePluginActivityCandidate(token, decision);
          return;
        } catch (error) {
          if (error instanceof ApiError && (error.status === 409 || error.status === 410)) return;
        }
      }
      if (candidateRef.current?.token === token) {
        // Force one clean document reload if the host decision request itself
        // could not reach the local server. The lifecycle deadline remains the
        // final fail-closed boundary.
        setCandidate(null);
      }
    },
    [closePort]
  );

  useEffect(() => {
    let stopped = false;
    let timer: number | null = null;
    let controller: AbortController | null = null;

    const poll = async () => {
      controller?.abort();
      controller = new AbortController();
      try {
        const catalog = await codeApi.pluginActivityCandidates(controller.signal);
        if (stopped) return;
        const next = selectCandidate(catalog.items);
        setCandidate((current) => (current?.token === next?.token ? current : next));
      } catch {
        // The mutation request owns the user-visible error. The preflight host
        // stays silent and lets the lifecycle timeout fail closed.
      } finally {
        if (!stopped) timer = window.setTimeout(poll, CANDIDATE_POLL_MS);
      }
    };

    void poll();
    return () => {
      stopped = true;
      controller?.abort();
      if (timer !== null) window.clearTimeout(timer);
    };
  }, []);

  useEffect(() => {
    closePort();
    loadedTokenRef.current = candidate?.token ?? null;
    loadCountRef.current = 0;
    decisionSentRef.current = false;
    return closePort;
  }, [candidate?.token, closePort]);

  if (!candidate) return null;

  const handleLoad = () => {
    if (candidateRef.current?.token !== candidate.token || decisionSentRef.current) return;
    if (loadedTokenRef.current !== candidate.token) {
      closePort();
      loadedTokenRef.current = candidate.token;
      loadCountRef.current = 0;
    }
    loadCountRef.current += 1;
    if (loadCountRef.current > 1) {
      void reportDecision(candidate.token, 'navigation-blocked');
      return;
    }
    const contentWindow = iframeRef.current?.contentWindow;
    if (!contentWindow) {
      void reportDecision(candidate.token, 'load-failed');
      return;
    }

    closePort();
    const channel = new MessageChannel();
    const port = channel.port1;
    portRef.current = port;
    port.onmessage = (event: MessageEvent<unknown>) => {
      if (portRef.current !== port || candidateRef.current?.token !== candidate.token) return;
      const message = parsePluginCandidateMessage(event.data);
      if (message?.type === 'ready') void reportDecision(candidate.token, 'ready');
      if (message?.type === 'error') void reportDecision(candidate.token, 'protocol-error');
    };
    port.onmessageerror = () => {
      if (portRef.current === port) void reportDecision(candidate.token, 'protocol-error');
    };
    port.start();
    readyTimeoutRef.current = window.setTimeout(() => {
      if (portRef.current === port) void reportDecision(candidate.token, 'timed-out');
    }, CANDIDATE_READY_TIMEOUT_MS);

    try {
      contentWindow.postMessage(
        activityCandidateHostInit(documentTheme(), navigator.language || 'en', candidate),
        '*',
        [channel.port2]
      );
    } catch {
      channel.port2.close();
      void reportDecision(candidate.token, 'protocol-error');
    }
  };

  return (
    <iframe
      key={candidate.token}
      ref={iframeRef}
      className='plugin-candidate-readiness-host'
      title={`${candidate.title || candidate.surfaceId} candidate readiness`}
      aria-hidden='true'
      tabIndex={-1}
      sandbox='allow-scripts'
      referrerPolicy='no-referrer'
      src={candidate.documentUrl}
      onLoad={handleLoad}
      onError={() => void reportDecision(candidate.token, 'load-failed')}
    />
  );
}

function selectCandidate(items: PluginUiCandidate[]): PluginUiCandidate | null {
  return [...items].filter(validCandidate).sort((left, right) => left.token.localeCompare(right.token))[0] ?? null;
}

function validCandidate(candidate: PluginUiCandidate): boolean {
  return (
    /^[a-f0-9]{64}$/.test(candidate.token) &&
    candidate.generation > 0 &&
    /^sha256:[a-f0-9]{64}$/.test(candidate.assetDigest) &&
    candidate.packageId.length > 0 &&
    /^[a-z][a-z0-9-]{0,62}$/.test(candidate.surfaceId) &&
    candidate.documentUrl === `/api/v1/plugins/activities/candidates/${candidate.token}/document`
  );
}

function documentTheme(): 'light' | 'dark' {
  return document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light';
}

function wait(milliseconds: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, milliseconds));
}
