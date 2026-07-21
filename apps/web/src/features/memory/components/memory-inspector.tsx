import {
  Activity,
  ChevronDown,
  ClipboardCopy,
  Clock3,
  Database,
  Eye,
  GitMerge,
  Hash,
  Info,
  Network,
  Sparkles,
  TriangleAlert,
  X,
} from 'lucide-react';
import { useEffect, useRef } from 'react';
import { navigateSettings, showToast } from '../../../state/app-state';
import type { MemoryEntry, MemoryGraphEntity, MemoryOverview } from '../../../types/api';
import {
  entityKindLabel,
  entityNameLabel,
  forgetSignalLabel,
  formatMemoryDate,
  memorySourceLabel,
  memoryTagLabel,
  memoryTypeLabel,
  percent,
  relationKindLabel,
  tierLabel,
} from '../memory-format';
import { entityForId, eventForMemory } from '../memory-projection';
import type { MemoryInspectorSelection } from '../memory-state';

export function MemoryInspector({
  data,
  selection,
  visibleMemoryIds,
  onClear,
  onSelectMemory,
  onSelectEntity,
}: {
  data: MemoryOverview;
  selection: MemoryInspectorSelection;
  visibleMemoryIds: Set<string>;
  onClear: () => void;
  onSelectMemory: (id: string) => void;
  onSelectEntity: (id: string) => void;
}) {
  const restoreFocusRef = useRef<Element | null>(null);
  const inspectorWasOpenRef = useRef(false);

  useEffect(() => {
    const inspectorOpen = selection !== null;
    if (inspectorOpen && !inspectorWasOpenRef.current) {
      const activeElement = document.activeElement;
      restoreFocusRef.current =
        activeElement && typeof (activeElement as HTMLElement).focus === 'function' ? activeElement : null;
    }
    inspectorWasOpenRef.current = inspectorOpen;
  }, [selection]);

  const clearAndRestoreFocus = () => {
    const focusTarget = restoreFocusRef.current;
    restoreFocusRef.current = null;
    onClear();
    if (focusTarget?.isConnected) (focusTarget as HTMLElement).focus();
  };

  if (!selection) return null;
  if (selection.kind === 'entity') {
    const entity = entityForId(data, selection.id);
    if (!entity) return null;
    return (
      <EntityInspector
        key={entity.id}
        data={data}
        entity={entity}
        visibleMemoryIds={visibleMemoryIds}
        onClear={clearAndRestoreFocus}
        onSelectMemory={onSelectMemory}
      />
    );
  }
  const entry = data.entries.find((item) => item.id === selection.id);
  if (!entry) return null;
  return (
    <MemoryEntryInspector
      key={entry.id}
      data={data}
      entry={entry}
      visible={visibleMemoryIds.has(entry.id)}
      onClear={clearAndRestoreFocus}
      onSelectEntity={onSelectEntity}
    />
  );
}

function MemoryEntryInspector({
  data,
  entry,
  visible,
  onClear,
  onSelectEntity,
}: {
  data: MemoryOverview;
  entry: MemoryEntry;
  visible: boolean;
  onClear: () => void;
  onSelectEntity: (id: string) => void;
}) {
  const facet = data.graph.facets[entry.id];
  const event = eventForMemory(data, entry.id);
  const entities = (facet?.entityIds ?? [])
    .map((id) => entityForId(data, id))
    .filter((entity): entity is MemoryGraphEntity => Boolean(entity));
  const metadata = Object.entries(entry.metadata ?? {});
  const source = memoryMetadata(entry, 'source') || event?.source || 'memory';
  const extractionReason = memoryMetadata(entry, 'reason');
  const confidence = memoryConfidence(entry);
  const scope = memoryMetadata(entry, 'scope');
  const workspace = memoryMetadata(entry, 'workspace');
  const tags = entry.tags.flatMap((tag) => {
    const label = memoryTagLabel(tag);
    return label ? [{ key: tag, label }] : [];
  });
  const content = entry.content || entry.preview || '没有可显示的内容。';
  const copyContent = async () => {
    try {
      await copyText(content);
      showToast('记忆内容已复制', 'success');
    } catch {
      showToast('无法复制记忆内容', 'error');
    }
  };
  return (
    <aside className='memory-inspector' aria-label='记忆详情'>
      <header>
        <div className='memory-inspector-title'>
          <span className='memory-inspector-kicker'>记忆详情</span>
          <h2>{event?.label || entry.preview || entry.id}</h2>
        </div>
        <div className='memory-inspector-actions'>
          <button type='button' aria-label='复制记忆' title='复制记忆' onClick={() => void copyContent()}>
            <ClipboardCopy size={14} />
          </button>
          <button type='button' aria-label='关闭详情' title='关闭详情' onClick={onClear}>
            <X size={15} />
          </button>
        </div>
      </header>
      {!visible && <p className='memory-inspector-filter-note'>这条记忆不在当前筛选结果中。</p>}
      <div className='memory-inspector-badges'>
        <span className='memory-type-badge' data-tone={entry.memoryType}>
          {memoryTypeLabel(entry.memoryType)}
        </span>
        {facet && (
          <>
            <span className='memory-tier-badge' data-tier={facet.tier}>
              {tierLabel(facet.tier)}
            </span>
            <span className='memory-signal-badge' data-signal={facet.forget}>
              {forgetSignalLabel(facet.forget)}
            </span>
          </>
        )}
      </div>
      <section className='memory-inspector-section'>
        <h3>内容</h3>
        <div className='memory-inspector-content'>{content}</div>
      </section>
      <section className='memory-inspector-section'>
        <h3>为什么会记住</h3>
        <div className='memory-retention-reason'>
          <p>{extractionReason || legacyMemoryReason(entry, source)}</p>
          <small>
            {extractionReason
              ? `由 LLM 在完整轮次结束后判断${confidence === null ? '' : ` · 置信度 ${percent(confidence)}`}。`
              : '这条旧版或手动记忆没有保存独立的 LLM 判断理由，页面不会根据关键词或统计数据代为猜测。'}
          </small>
        </div>
      </section>
      {facet && (
        <section className='memory-inspector-section'>
          <div className='memory-retention-heading'>
            <h3>保留优先级</h3>
            <strong>{percent(facet.retentionScore)}</strong>
          </div>
          <meter
            className='memory-retention-meter'
            aria-label='保留优先级'
            min={0}
            max={1}
            value={facet.retentionScore}
          >
            {percent(facet.retentionScore)}
          </meter>
          <p className='memory-retention-help'>这是系统综合重要度、时间和访问情况计算的只读参考值。</p>
          <div className='memory-lifecycle-badges'>
            {facet.llmExtracted && (
              <span>
                <Sparkles size={12} /> 自动提取
              </span>
            )}
            {facet.consolidated && (
              <span>
                <GitMerge size={12} /> 已合并重复
              </span>
            )}
            {facet.conflicts && (
              <span className='attention'>
                <TriangleAlert size={12} /> 待处理冲突
              </span>
            )}
            {!facet.llmExtracted && !facet.consolidated && !facet.conflicts && <span>直接记录</span>}
          </div>
        </section>
      )}
      <section className='memory-inspector-section'>
        <h3>活动</h3>
        <dl className='memory-detail-grid'>
          <div>
            <dt>
              <Clock3 size={12} />
              写入
            </dt>
            <dd>{formatMemoryDate(entry.timestamp)}</dd>
          </div>
          <div>
            <dt>
              <Eye size={12} />
              访问
            </dt>
            <dd>{entry.accessCount} 次</dd>
          </div>
          <div>
            <dt>
              <Activity size={12} />
              最近访问
            </dt>
            <dd>{formatMemoryDate(entry.lastAccessed)}</dd>
          </div>
          <div>
            <dt>
              <Hash size={12} />
              重要度
            </dt>
            <dd>{percent(entry.importance)}</dd>
          </div>
          <div>
            <dt>
              <Database size={12} />
              来源
            </dt>
            <dd>{memorySourceLabel(source)}</dd>
          </div>
          {confidence !== null && (
            <div>
              <dt>
                <Sparkles size={12} />
                LLM 置信度
              </dt>
              <dd>{percent(confidence)}</dd>
            </div>
          )}
          {scope && (
            <div>
              <dt>
                <Network size={12} />
                生效范围
              </dt>
              <dd>{memoryScopeLabel(scope)}</dd>
            </div>
          )}
          {workspace && (
            <div>
              <dt>
                <Network size={12} />
                记录工作区
              </dt>
              <dd title={workspace}>{workspace}</dd>
            </div>
          )}
        </dl>
      </section>
      {tags.length > 0 && (
        <section className='memory-inspector-section'>
          <h3>标签</h3>
          <div className='memory-inspector-tags'>
            {tags.map((tag) => (
              <span className='memory-tag' key={tag.key}>
                {tag.label}
              </span>
            ))}
          </div>
        </section>
      )}
      {entities.length > 0 && (
        <section className='memory-inspector-section'>
          <h3>
            关联实体 <small>{entities.length}</small>
          </h3>
          <div className='memory-entity-links'>
            {entities.map((entity) => (
              <button type='button' key={entity.id} onClick={() => onSelectEntity(entity.id)}>
                <i data-tone={entity.kind} />
                <span>
                  <strong>{entityNameLabel(entity.kind, entity.name)}</strong>
                  <small>{entityKindLabel(entity.kind)}</small>
                </span>
              </button>
            ))}
          </div>
        </section>
      )}
      <details className='memory-technical-details'>
        <summary>
          <span>
            技术信息
            <small>元数据与记忆 ID</small>
          </span>
          <ChevronDown size={14} aria-hidden='true' />
        </summary>
        {metadata.length > 0 && (
          <dl className='memory-metadata'>
            {metadata.map(([key, value]) => (
              <div key={key}>
                <dt>{key}</dt>
                <dd>{value}</dd>
              </div>
            ))}
          </dl>
        )}
        <dl className='memory-metadata memory-identity'>
          <div>
            <dt>memory_id</dt>
            <dd>
              <code title={entry.id}>{entry.id}</code>
            </dd>
          </div>
        </dl>
      </details>
      <footer className='memory-inspector-readonly'>
        <Info size={13} aria-hidden='true' />
        <span>
          此页面仅供查看，不会修改或删除记忆。
          <button type='button' onClick={() => navigateSettings('context')}>
            调整记忆设置
          </button>
        </span>
      </footer>
    </aside>
  );
}

function EntityInspector({
  data,
  entity,
  visibleMemoryIds,
  onClear,
  onSelectMemory,
}: {
  data: MemoryOverview;
  entity: MemoryGraphEntity;
  visibleMemoryIds: Set<string>;
  onClear: () => void;
  onSelectMemory: (id: string) => void;
}) {
  const entries = entity.memoryIds
    .map((id) => data.entries.find((entry) => entry.id === id))
    .filter((entry): entry is MemoryEntry => Boolean(entry))
    .sort((left, right) => Date.parse(right.timestamp) - Date.parse(left.timestamp));
  const relations = data.graph.relations.filter((relation) => relation.from === entity.id || relation.to === entity.id);
  const relationKinds = new Set(relations.map((relation) => relation.kind));
  return (
    <aside className='memory-inspector' aria-label='实体详情'>
      <header>
        <div className='memory-inspector-title'>
          <span className='memory-inspector-kicker'>{entityKindLabel(entity.kind)}实体</span>
          <h2>{entityNameLabel(entity.kind, entity.name)}</h2>
        </div>
        <div className='memory-inspector-actions'>
          <button type='button' aria-label='关闭详情' title='关闭详情' onClick={onClear}>
            <X size={15} />
          </button>
        </div>
      </header>
      <span className='memory-entity-hero' data-tone={entity.kind}>
        <Network size={18} />
      </span>
      <dl className='memory-detail-grid entity'>
        <div>
          <dt>提及</dt>
          <dd>{entity.mentions} 次</dd>
        </div>
        <div>
          <dt>重要度</dt>
          <dd>{percent(entity.importance)}</dd>
        </div>
        <div>
          <dt>相关记忆</dt>
          <dd>{entity.memoryIds.length} 条</dd>
        </div>
        <div>
          <dt>关系</dt>
          <dd>{relations.length} 条</dd>
        </div>
        <div>
          <dt>首次出现</dt>
          <dd>{formatMemoryDate(entity.firstSeen)}</dd>
        </div>
        <div>
          <dt>最近出现</dt>
          <dd>{formatMemoryDate(entity.lastSeen)}</dd>
        </div>
      </dl>
      {entity.aliases.length > 0 && (
        <section className='memory-inspector-section'>
          <h3>别名</h3>
          <div className='memory-inspector-tags'>
            {entity.aliases.map((alias) => (
              <span className='memory-tag' key={alias}>
                {alias}
              </span>
            ))}
          </div>
        </section>
      )}
      {relationKinds.size > 0 && (
        <section className='memory-inspector-section'>
          <h3>关系类型</h3>
          <div className='memory-inspector-tags'>
            {[...relationKinds].map((kind) => (
              <span className='memory-tag' key={kind}>
                {relationKindLabel(kind)}
              </span>
            ))}
          </div>
        </section>
      )}
      <section className='memory-inspector-section entity-memories'>
        <h3>
          相关记忆 <small>{entries.length}</small>
        </h3>
        <div className='memory-entity-memory-list'>
          {entries.map((entry) => (
            <button
              type='button'
              key={entry.id}
              data-filtered={!visibleMemoryIds.has(entry.id) || undefined}
              onClick={() => onSelectMemory(entry.id)}
            >
              <strong>{eventForMemory(data, entry.id)?.label || entry.preview}</strong>
              <span>
                {memoryTypeLabel(entry.memoryType)} · {formatMemoryDate(entry.timestamp)}
              </span>
            </button>
          ))}
          {!entries.length && <p>相关记忆不在当前加载窗口中。</p>}
        </div>
      </section>
      <details className='memory-technical-details'>
        <summary>
          <span>
            技术信息
            <small>实体 ID</small>
          </span>
          <ChevronDown size={14} aria-hidden='true' />
        </summary>
        <dl className='memory-metadata memory-identity'>
          <div>
            <dt>entity_id</dt>
            <dd>
              <code title={entity.id}>{entity.id}</code>
            </dd>
          </div>
        </dl>
      </details>
    </aside>
  );
}

function memoryMetadata(entry: MemoryEntry, key: string): string | undefined {
  const value = entry.metadata?.[key]?.trim();
  return value || undefined;
}

function memoryConfidence(entry: MemoryEntry): number | null {
  const value = Number(memoryMetadata(entry, 'confidence'));
  return Number.isFinite(value) && value >= 0 && value <= 1 ? value : null;
}

function memoryScopeLabel(scope: string): string {
  if (scope === 'workspace') return '当前工作区';
  if (scope === 'user') return '用户全局';
  return scope;
}

function legacyMemoryReason(entry: MemoryEntry, source: string): string {
  return `这条${memoryTypeLabel(entry.memoryType)}来自「${memorySourceLabel(source)}」。`;
}

async function copyText(content: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(content);
    return;
  }

  const textarea = document.createElement('textarea');
  textarea.value = content;
  textarea.setAttribute('readonly', '');
  textarea.style.position = 'fixed';
  textarea.style.opacity = '0';
  document.body.append(textarea);
  textarea.select();
  const copied = document.execCommand?.('copy') ?? false;
  textarea.remove();
  if (!copied) throw new Error('Clipboard is unavailable');
}
