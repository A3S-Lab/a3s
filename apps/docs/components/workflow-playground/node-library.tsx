import { useMemo, useState } from 'react';
import { MagnifyingGlass, X } from '@phosphor-icons/react';
import type { WorkflowPlaygroundCopy } from './workflow-copy';
import { workflowIconByProfile } from './workflow-icons';
import {
  workflowCatalog,
  type PlaygroundLang,
  type WorkflowNodeGroup,
  type WorkflowNodeProfile,
} from './workflow-model';

const groupOrder: WorkflowNodeGroup[] = ['core', 'intelligence', 'logic', 'transform', 'integration', 'trigger', 'human'];

interface NodeLibraryProps {
  copy: WorkflowPlaygroundCopy;
  lang: PlaygroundLang;
  open: boolean;
  onClose: () => void;
  onSelect: (profile: WorkflowNodeProfile) => void;
}

export function NodeLibrary({ copy, lang, open, onClose, onSelect }: NodeLibraryProps) {
  const [query, setQuery] = useState('');
  const [tab, setTab] = useState<'nodes' | 'tools'>('nodes');
  const filtered = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    return workflowCatalog.filter((item) => {
      const matchesTab = tab === 'tools'
        ? item.profile === 'tool'
        : item.profile !== 'tool';
      if (!matchesTab) return false;
      if (!normalized) return true;
      return item.name[lang].toLocaleLowerCase().includes(normalized)
        || item.description[lang].toLocaleLowerCase().includes(normalized)
        || item.profile.includes(normalized)
        || item.semanticProfile.includes(normalized);
    });
  }, [lang, query, tab]);

  if (!open) return null;

  return (
    <aside className="a3s-node-library" aria-label={copy.nodeLibrary} data-testid="node-library">
      <header>
        <div>
          <h2>{copy.nodeLibrary}</h2>
          <p>{copy.nodeLibraryDescription}</p>
        </div>
        <button type="button" onClick={onClose} aria-label={copy.close} title={copy.close}>
          <X aria-hidden="true" />
        </button>
      </header>

      <nav className="a3s-node-library__tabs" aria-label={copy.nodeLibrary}>
        <button type="button" className={tab === 'nodes' ? 'is-active' : undefined} aria-pressed={tab === 'nodes'} onClick={() => setTab('nodes')}>{copy.nodesTab}</button>
        <button type="button" className={tab === 'tools' ? 'is-active' : undefined} aria-pressed={tab === 'tools'} onClick={() => setTab('tools')}>{copy.toolsTab}</button>
      </nav>

      <label className="a3s-node-library__search">
        <MagnifyingGlass aria-hidden="true" />
        <span className="a3s-visually-hidden">{copy.searchNodes}</span>
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.currentTarget.value)}
          placeholder={copy.searchNodes}
          autoFocus
        />
      </label>

      <div className="a3s-node-library__groups">
        {groupOrder.map((group) => {
          const items = filtered.filter((item) => item.group === group);
          if (items.length === 0) return null;
          return (
            <section key={group} aria-labelledby={`workflow-group-${group}`}>
              <h3 id={`workflow-group-${group}`}>{copy.groups[group]}</h3>
              <div>
                {items.map((item) => {
                  const Icon = workflowIconByProfile[item.profile];
                  return (
                    <button
                      type="button"
                      key={item.profile}
                      onClick={() => onSelect(item.profile)}
                      data-node-profile={item.profile}
                    >
                      <span aria-hidden="true"><Icon weight="duotone" /></span>
                      <span><strong>{item.name[lang]}</strong><small>{item.description[lang]}</small><code>{item.semanticProfile}</code></span>
                    </button>
                  );
                })}
              </div>
            </section>
          );
        })}
        {filtered.length === 0 ? <p className="a3s-node-library__empty">{copy.noNodes}</p> : null}
      </div>
    </aside>
  );
}
