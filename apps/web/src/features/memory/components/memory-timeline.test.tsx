import { fireEvent, render } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { memoryTestData } from '../memory-test-data';
import { MemoryTimeline } from './memory-timeline';

describe('MemoryTimeline', () => {
  it('renders large timelines in explicit batches', () => {
    const template = memoryTestData();
    const entries = Array.from({ length: 65 }, (_, index) => ({
      ...template.entries[0],
      id: `memory-${index}`,
      content: `Memory item number ${index} with supporting details`,
      preview: `Memory item number ${index} with supporting details`,
      tags: [],
      timestamp: new Date(Date.parse('2026-07-20T08:00:00Z') - index * 60_000).toISOString(),
    }));
    const events = entries.map((entry, index) => ({
      ...template.graph.events[0],
      id: `event:${entry.id}`,
      memoryId: entry.id,
      label: `Memory item number ${index}`,
      timestamp: entry.timestamp,
      entityIds: [],
      retentionScore: 0.9 - index / 1_000,
    }));
    const data = {
      ...template,
      entries,
      stats: { ...template.stats, entries: entries.length },
      graph: { ...template.graph, events, facets: {} },
    };

    const { container } = render(<MemoryTimeline data={data} entries={entries} onSelectMemory={vi.fn()} />);

    const timeline = container.querySelector('.memory-timeline');
    if (!timeline) throw new Error('Timeline did not render');
    expect(timeline.querySelectorAll('.memory-timeline-entry')).toHaveLength(60);
    expect(timeline.querySelectorAll('.memory-timeline-copy > p')).toHaveLength(0);
    expect(timeline.querySelector('.memory-timeline-pagination')).toHaveTextContent('60 / 65 条');
    expect(timeline.querySelector('.memory-source')).toHaveTextContent('偏好');

    const continueButton = timeline.querySelector('.memory-timeline-pagination button');
    expect(continueButton).not.toBeNull();
    fireEvent.click(continueButton as HTMLButtonElement);

    expect(timeline.querySelectorAll('.memory-timeline-entry')).toHaveLength(65);
    expect(timeline.querySelector('.memory-timeline-pagination')).not.toBeInTheDocument();
  });
});
