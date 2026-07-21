import { Bookmark, Clock3, GitMerge, Sparkles, TriangleAlert } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import type { MemoryEntry, MemoryOverview } from '../../../types/api';
import {
  forgetSignalLabel,
  memoryDayLabel,
  memorySourceLabel,
  memoryTagLabel,
  memoryTypeLabel,
  percent,
  relativeMemoryTime,
  tierLabel,
} from '../memory-format';

const TIMELINE_BATCH_SIZE = 60;

export function MemoryTimeline({
  data,
  entries,
  selectedMemoryId,
  onSelectMemory,
}: {
  data: MemoryOverview;
  entries: MemoryEntry[];
  selectedMemoryId?: string;
  onSelectMemory: (id: string) => void;
}) {
  const [visibleCount, setVisibleCount] = useState(TIMELINE_BATCH_SIZE);
  const sortedEntries = useMemo(
    () => [...entries].sort((left, right) => Date.parse(right.timestamp) - Date.parse(left.timestamp)),
    [entries]
  );
  const visibleEntries = useMemo(() => sortedEntries.slice(0, visibleCount), [sortedEntries, visibleCount]);
  const groups = useMemo(() => groupEntries(visibleEntries), [visibleEntries]);
  const events = useMemo(() => new Map(data.graph.events.map((event) => [event.memoryId, event])), [data]);

  useEffect(() => setVisibleCount(TIMELINE_BATCH_SIZE), [entries]);

  useEffect(() => {
    if (!selectedMemoryId) return;
    const selectedIndex = sortedEntries.findIndex((entry) => entry.id === selectedMemoryId);
    if (selectedIndex < 0) return;
    const requiredCount = Math.ceil((selectedIndex + 1) / TIMELINE_BATCH_SIZE) * TIMELINE_BATCH_SIZE;
    setVisibleCount((current) => Math.max(current, requiredCount));
  }, [selectedMemoryId, sortedEntries]);

  return (
    <section className='memory-timeline' aria-label='记忆时间线'>
      {groups.map(([day, items]) => (
        <section className='memory-timeline-day' key={day}>
          <header>
            <span>{memoryDayLabel(day)}</span>
            <small>{items.length} 条</small>
          </header>
          <ul>
            {items.map((entry) => {
              const facet = data.graph.facets[entry.id];
              const event = events.get(entry.id);
              const title = event?.label || entry.preview || entry.id;
              const preview = distinctTimelinePreview(title, entry.preview);
              const tags = entry.tags.flatMap((tag) => {
                const label = memoryTagLabel(tag);
                return label ? [{ key: tag, label }] : [];
              });
              return (
                <li key={entry.id}>
                  <button
                    type='button'
                    className={`memory-timeline-entry ${selectedMemoryId === entry.id ? 'selected' : ''}`}
                    aria-current={selectedMemoryId === entry.id ? 'true' : undefined}
                    onClick={() => onSelectMemory(entry.id)}
                  >
                    <span className='memory-timeline-node' data-tone={entry.memoryType} aria-hidden='true' />
                    <span className='memory-timeline-copy'>
                      <span className='memory-timeline-meta'>
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
                        <time dateTime={entry.timestamp}>{relativeMemoryTime(entry.timestamp)}</time>
                      </span>
                      <strong>{title}</strong>
                      {preview && <p>{preview}</p>}
                      <span className='memory-timeline-footer'>
                        <span className='memory-importance' title={`重要度 ${percent(entry.importance)}`}>
                          <i style={{ width: percent(entry.importance) }} />
                        </span>
                        {tags.slice(0, 4).map((tag) => (
                          <span className='memory-tag' key={tag.key}>
                            {tag.label}
                          </span>
                        ))}
                        {tags.length > 4 && <span className='memory-tag'>+{tags.length - 4}</span>}
                        {facet?.llmExtracted && <Sparkles size={12} aria-label='自动提取' />}
                        {facet?.consolidated && <GitMerge size={12} aria-label='已合并重复' />}
                        {facet?.conflicts && <TriangleAlert size={12} aria-label='待处理冲突' />}
                        {facet?.forget === 'protected' && <Bookmark size={12} aria-label='重点保留' />}
                        <span className='memory-source'>
                          <Clock3 size={11} /> {memorySourceLabel(event?.source || 'memory')}
                        </span>
                      </span>
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        </section>
      ))}
      <footer className='memory-timeline-pagination' aria-live='polite'>
        <span>
          已显示 {visibleEntries.length} / {entries.length} 条
        </span>
        {visibleEntries.length < entries.length && (
          <button
            type='button'
            onClick={() => setVisibleCount((count) => Math.min(entries.length, count + TIMELINE_BATCH_SIZE))}
          >
            继续显示
          </button>
        )}
      </footer>
    </section>
  );
}

function groupEntries(entries: MemoryEntry[]): Array<[string, MemoryEntry[]]> {
  const groups = new Map<string, MemoryEntry[]>();
  for (const entry of entries) {
    const timestamp = new Date(entry.timestamp);
    const key = Number.isNaN(timestamp.getTime())
      ? 'invalid'
      : `${timestamp.getFullYear()}-${String(timestamp.getMonth() + 1).padStart(2, '0')}-${String(
          timestamp.getDate()
        ).padStart(2, '0')}T00:00:00`;
    const items = groups.get(key) ?? [];
    items.push(entry);
    groups.set(key, items);
  }
  return [...groups];
}

function distinctTimelinePreview(title: string, preview: string): string | null {
  if (!preview.trim()) return null;
  const normalize = (value: string) =>
    value
      .trim()
      .toLocaleLowerCase()
      .replace(/[\s.,!?;:,。！？；：“”"'()[\]{}_—-]+/g, '');
  const normalizedTitle = normalize(title);
  const normalizedPreview = normalize(preview);
  if (!normalizedTitle || normalizedTitle === normalizedPreview) return null;
  if (
    (normalizedPreview.startsWith(normalizedTitle) || normalizedTitle.startsWith(normalizedPreview)) &&
    Math.min(normalizedTitle.length, normalizedPreview.length) >= 12
  ) {
    return null;
  }
  return preview;
}
