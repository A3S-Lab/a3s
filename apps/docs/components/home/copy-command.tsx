'use client';

import { Check, Copy } from '@phosphor-icons/react';
import { useEffect, useState } from 'react';

interface CopyCommandProps {
  command: string;
  copyLabel: string;
  copiedLabel: string;
}

export function CopyCommand({ command, copyLabel, copiedLabel }: CopyCommandProps) {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1800);
    return () => window.clearTimeout(timer);
  }, [copied]);

  async function copy() {
    try {
      await navigator.clipboard.writeText(command);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  }

  return (
    <button
      aria-label={copied ? copiedLabel : copyLabel}
      aria-live="polite"
      className="a3s-copy-command"
      onClick={copy}
      type="button"
    >
      {copied ? <Check aria-hidden="true" /> : <Copy aria-hidden="true" />}
      <span>{copied ? copiedLabel : copyLabel}</span>
    </button>
  );
}
