import { History, RotateCcw } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Dialog } from '../../../design-system/primitives/dialog/dialog';
import type { WorkActions } from '../use-work-controller';
import type { WorkArtifactVersion } from '../work-types';

export function WorkVersionHistoryDialog({ actions, onClose }: { actions: WorkActions; onClose: () => void }) {
  const [versions, setVersions] = useState<WorkArtifactVersion[]>([]);
  const [loading, setLoading] = useState(true);
  const [restoring, setRestoring] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let current = true;
    void actions
      .artifactVersions()
      .then((items) => {
        if (current) setVersions(items);
      })
      .catch((reason: unknown) => {
        if (current) setError(reason instanceof Error ? reason.message : '无法读取版本历史');
      })
      .finally(() => {
        if (current) setLoading(false);
      });
    return () => {
      current = false;
    };
  }, [actions]);

  return (
    <Dialog
      title='版本历史'
      description={
        actions.storageMode === 'server' ? '每次成功保存都会生成可恢复的服务器版本。' : '本地模式仅保留当前版本。'
      }
      onClose={onClose}
      closeDisabled={restoring !== null}
    >
      <section className='work-version-history' aria-live='polite'>
        {loading ? (
          <output className='work-version-state'>正在读取版本…</output>
        ) : error ? (
          <div className='work-version-state error'>{error}</div>
        ) : (
          <ol>
            {versions.map((version) => (
              <li key={`${version.revision}-${version.updatedAt}`}>
                <span className='work-version-icon'>
                  <History size={15} />
                </span>
                <span>
                  <strong>第 {version.revision} 版</strong>
                  <small>{formatVersionTime(version.updatedAt)}</small>
                </span>
                {version.current ? (
                  <em>当前版本</em>
                ) : (
                  <button
                    type='button'
                    disabled={restoring !== null}
                    onClick={async () => {
                      setRestoring(version.revision);
                      if (await actions.restoreVersion(version.revision)) onClose();
                      else setRestoring(null);
                    }}
                  >
                    <RotateCcw size={13} />
                    {restoring === version.revision ? '正在恢复…' : '恢复'}
                  </button>
                )}
              </li>
            ))}
          </ol>
        )}
      </section>
    </Dialog>
  );
}

function formatVersionTime(timestamp: number): string {
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  }).format(timestamp);
}
