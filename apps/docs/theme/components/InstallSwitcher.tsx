import { useState } from 'react';

type Locale = 'zh' | 'en';

const installers = [
  {
    id: 'shell',
    label: 'Shell',
    command:
      "curl --proto '=https' --tlsv1.2 -LsSf https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh",
  },
  {
    id: 'homebrew',
    label: 'Homebrew',
    command: 'brew install a3s-lab/tap/a3s',
  },
  {
    id: 'windows',
    label: 'PowerShell',
    command:
      'irm https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.ps1 | iex',
  },
  {
    id: 'cargo',
    label: 'Cargo',
    command: 'cargo install a3s',
  },
] as const;

export function InstallSwitcher({ locale }: { locale: Locale }) {
  const [activeId, setActiveId] =
    useState<(typeof installers)[number]['id']>('shell');
  const [copied, setCopied] = useState(false);
  const active =
    installers.find((installer) => installer.id === activeId) ?? installers[0];

  async function copyCommand() {
    await navigator.clipboard.writeText(active.command);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1400);
  }

  return (
    <div className="cli-install">
      <div
        className="cli-install__tabs"
        role="tablist"
        aria-label="Install method"
      >
        {installers.map((installer) => (
          <button
            aria-selected={installer.id === activeId}
            key={installer.id}
            onClick={() => {
              setActiveId(installer.id);
              setCopied(false);
            }}
            role="tab"
            type="button"
          >
            {installer.label}
          </button>
        ))}
      </div>
      <div className="cli-install__command">
        <span aria-hidden="true">$</span>
        <code>{active.command}</code>
        <button onClick={copyCommand} type="button">
          {copied
            ? locale === 'zh'
              ? '已复制'
              : 'Copied'
            : locale === 'zh'
              ? '复制'
              : 'Copy'}
        </button>
      </div>
    </div>
  );
}
