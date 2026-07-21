import { BrainCircuit, GitMerge, Network, ShieldCheck, Sparkles, TriangleAlert } from 'lucide-react';
import type { MemoryOverview } from '../../../types/api';

export function MemorySummary({ data, visibleCount }: { data: MemoryOverview; visibleCount: number }) {
  const stats = data.graph.stats;
  const retentionTotal = Math.max(1, stats.short + stats.mid + stats.long);
  return (
    <section className='memory-summary' aria-label='记忆概览'>
      <article>
        <span className='memory-summary-icon event'>
          <BrainCircuit size={17} />
        </span>
        <div>
          <strong>{data.stats.entries.toLocaleString()}</strong>
          <span>记忆总数</span>
          <small>{visibleCount === data.stats.entries ? '全部可见' : `当前显示 ${visibleCount}`}</small>
        </div>
      </article>
      <article>
        <span className='memory-summary-icon entity'>
          <Network size={17} />
        </span>
        <div>
          <strong>{stats.entities.toLocaleString()}</strong>
          <span>知识实体</span>
          <small>{stats.aliases} 个别名已合并</small>
        </div>
      </article>
      <article>
        <span className='memory-summary-icon relation'>
          <GitMerge size={17} />
        </span>
        <div>
          <strong>{stats.relations.toLocaleString()}</strong>
          <span>语义关联</span>
          <small>来自 {stats.events} 个记忆事件</small>
        </div>
      </article>
      <article className='memory-retention-card'>
        <span className='memory-summary-icon retention'>
          <ShieldCheck size={17} />
        </span>
        <div>
          <strong>{stats.long.toLocaleString()}</strong>
          <span>长期保留</span>
          <div
            className='memory-retention-bar'
            role='img'
            aria-label={`短期 ${stats.short}，中期 ${stats.mid}，长期 ${stats.long}`}
          >
            <i className='short' style={{ width: `${(stats.short / retentionTotal) * 100}%` }} />
            <i className='mid' style={{ width: `${(stats.mid / retentionTotal) * 100}%` }} />
            <i className='long' style={{ width: `${(stats.long / retentionTotal) * 100}%` }} />
          </div>
        </div>
      </article>
      <div className='memory-summary-signals'>
        <strong>记忆状态</strong>
        <span>
          <Sparkles size={12} /> 自动提取 {stats.llmExtracted}
        </span>
        <span>
          <GitMerge size={12} /> 已合并 {stats.consolidated}
        </span>
        <span className={stats.conflicts ? 'attention' : undefined}>
          <TriangleAlert size={12} /> 待处理冲突 {stats.conflicts}
        </span>
        <span
          className={stats.forgetCandidates ? 'attention' : undefined}
          title='这是基于保留信号生成的建议，不会自动删除记忆。'
        >
          建议清理 {stats.forgetCandidates}
        </span>
        <span>重点记忆 {data.stats.important}</span>
        <span>{data.stats.tags} 个标签</span>
      </div>
    </section>
  );
}
