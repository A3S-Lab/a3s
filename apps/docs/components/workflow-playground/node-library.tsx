import { useMemo, useState } from 'react';
import { MagnifyingGlass, X } from '@phosphor-icons/react';
import type { WorkflowPlaygroundCopy } from './workflow-copy';
import { workflowIconByKind } from './workflow-icons';
import {
  workflowCatalog,
  type PlaygroundLang,
  type WorkflowNodeGroup,
  type WorkflowStepKind,
} from './workflow-model';

const groupOrder: WorkflowNodeGroup[] = ['flow', 'intelligence', 'integration', 'human'];

interface NodeLibraryProps {
  copy: WorkflowPlaygroundCopy;
  lang: PlaygroundLang;
  open: boolean;
  onClose: () => void;
  onSelect: (kind: WorkflowStepKind) => void;
}

export function NodeLibrary({ copy, lang, open, onClose, onSelect }: NodeLibraryProps) {
  const [query, setQuery] = useState('');
  const filtered = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    if (!normalized) return workflowCatalog;
    return workflowCatalog.filter((item) => (
      item.name[lang].toLocaleLowerCase().includes(normalized)
      || item.description[lang].toLocaleLowerCase().includes(normalized)
      || item.kind.includes(normalized)
    ));
  }, [lang, query]);

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
                  const Icon = workflowIconByKind[item.kind];
                  return (
                    <button
                      type="button"
                      key={item.kind}
                      onClick={() => onSelect(item.kind)}
                      data-node-kind={item.kind}
                    >
                      <span aria-hidden="true"><Icon weight="duotone" /></span>
                      <span><strong>{item.name[lang]}</strong><small>{item.description[lang]}</small></span>
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
