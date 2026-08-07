'use client';

import { useState } from 'react';
import { CopyCommand } from '@/components/home/copy-command';
import { homeContent, type Lang } from '@/components/home/home-content';

type InstallerId = (typeof homeContent.en.quickstart.installers)[number]['id'];

export function InstallCommandTerminal({ lang }: { lang: Lang }) {
  const tr = homeContent[lang].quickstart;
  const [activeId, setActiveId] = useState<InstallerId>('unix');
  const activeInstaller = tr.installers.find((installer) => installer.id === activeId) ?? tr.installers[0];

  return (
    <div className="a3s-terminal-card">
      <div className="a3s-terminal-card__bar">
        <span><i /><i /><i /></span>
        <code>{activeInstaller.shell}</code>
        <CopyCommand
          command={activeInstaller.command}
          copiedLabel={tr.copied}
          copyLabel={tr.copy}
        />
      </div>
      <div
        aria-label={lang === 'cn' ? '选择安装方式' : 'Choose an installation method'}
        className="a3s-terminal-card__tabs"
        role="group"
      >
        {tr.installers.map((installer) => (
          <button
            aria-pressed={activeId === installer.id}
            key={installer.id}
            onClick={() => setActiveId(installer.id)}
            type="button"
          >
            {installer.label}
          </button>
        ))}
      </div>
      <div className="a3s-terminal-card__panels">
        {tr.installers.map((installer) => (
          <pre hidden={activeId !== installer.id} key={installer.id}><code>{installer.command}</code></pre>
        ))}
      </div>
      <div className="a3s-terminal-card__status">
        <span><i /> {lang === 'cn' ? '安装器就绪' : 'installer ready'}</span>
        <span>{activeInstaller.shell}</span>
      </div>
    </div>
  );
}
