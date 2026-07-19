import { Box, LoaderCircle, Rotate3D, ScanLine } from 'lucide-react';
import type { ReactNode } from 'react';
import type { HangarAirframeOption, HangarDraft, HangarPilotOption } from '../hangar-configuration';

export type HangarPreviewStatus = 'ready' | 'loading' | 'unavailable';

export interface HangarPreviewProps {
  airframe: HangarAirframeOption;
  pilot: HangarPilotOption;
  draft: Readonly<HangarDraft>;
  children?: ReactNode;
  status?: HangarPreviewStatus;
  onResetView?: () => void;
}

const STATUS_COPY: Record<HangarPreviewStatus, string> = {
  ready: '3D 场景就绪',
  loading: '正在装载 3D 机体',
  unavailable: '等待 3D 场景接入',
};

export function HangarPreview({
  airframe,
  pilot,
  draft,
  children,
  status = children ? 'ready' : 'unavailable',
  onResetView,
}: HangarPreviewProps) {
  return (
    <section className='hangar-preview' aria-label={`${airframe.displayName} 3D 预览`}>
      <header className='hangar-preview__header'>
        <div>
          <span className='hangar-preview__kicker'>AIRFRAME BAY / LIVE SPECIMEN</span>
          <strong>{airframe.displayName}</strong>
        </div>
        <output className={`hangar-preview__status is-${status}`}>
          {status === 'loading' ? <LoaderCircle className='spin' size={13} /> : <ScanLine size={13} />}
          {STATUS_COPY[status]}
        </output>
      </header>

      <div className='hangar-preview__viewport'>
        <div className='hangar-preview__stage-grid' aria-hidden='true' />
        {children ? (
          <div className='hangar-preview__content'>{children}</div>
        ) : (
          <output className='hangar-preview__empty'>
            <span className='hangar-preview__empty-icon' aria-hidden='true'>
              <Box size={25} strokeWidth={1.4} />
            </span>
            <strong>等待 3D 预览渲染器</strong>
            <span>父层可在 preview 插槽中挂载 Three.js 场景；这里不会使用二维飞机替代。</span>
          </output>
        )}

        <div className='hangar-preview__axis' aria-hidden='true'>
          <span>X</span>
          <i />
          <span>Z</span>
        </div>
        <div className='hangar-preview__reticle' aria-hidden='true'>
          <i />
        </div>

        <div className='hangar-preview__identity'>
          <span>{draft.callsign || 'UNASSIGNED'}</span>
          <strong>{airframe.id.toUpperCase()}</strong>
          <small>
            {pilot.displayName} · {draft.model || 'MODEL NOT SET'}
          </small>
        </div>

        <dl className='hangar-preview__telemetry'>
          <div>
            <dt>PILOT</dt>
            <dd>{pilot.id.toUpperCase()}</dd>
          </div>
          <div>
            <dt>EFFORT</dt>
            <dd>{draft.effort.toUpperCase()}</dd>
          </div>
          <div>
            <dt>ROLE</dt>
            <dd>{airframe.role.replaceAll('-', ' ').toUpperCase()}</dd>
          </div>
        </dl>
      </div>

      <footer className='hangar-preview__footer'>
        <span>
          <Rotate3D size={14} aria-hidden='true' /> 拖拽旋转 · 双击复位
        </span>
        {onResetView ? (
          <button type='button' onClick={onResetView}>
            重置视角
          </button>
        ) : null}
      </footer>
    </section>
  );
}
