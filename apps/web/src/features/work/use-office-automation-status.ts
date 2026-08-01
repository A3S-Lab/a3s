import { useEffect, useState } from 'react';
import { codeApi } from '../../lib/api';
import type { OfficeAutomationStatus } from '../../types/api';

const PREPARING_REFRESH_MS = 2_000;
const SETTLED_REFRESH_MS = 15_000;

export function useOfficeAutomationStatus(): OfficeAutomationStatus | null {
  const [status, setStatus] = useState<OfficeAutomationStatus | null>(null);

  useEffect(() => {
    let disposed = false;
    let timer: number | null = null;
    let request: AbortController | null = null;

    const refresh = async () => {
      request = new AbortController();
      let delay = PREPARING_REFRESH_MS;
      try {
        const next = await codeApi.officeAutomationStatus(request.signal);
        if (disposed) return;
        setStatus(next);
        delay = next.status === 'preparing' ? PREPARING_REFRESH_MS : SETTLED_REFRESH_MS;
      } catch {
        if (disposed || request.signal.aborted) return;
        delay = PREPARING_REFRESH_MS;
      }
      if (!disposed) timer = window.setTimeout(refresh, delay);
    };

    void refresh();
    return () => {
      disposed = true;
      request?.abort();
      if (timer !== null) window.clearTimeout(timer);
    };
  }, []);

  return status;
}
