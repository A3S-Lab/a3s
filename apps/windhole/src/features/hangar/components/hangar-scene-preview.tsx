import { useEffect, useMemo, useRef, useState } from 'react';
import {
  createHangarPreviewRuntime,
  type HangarPreviewConfiguration,
  type HangarPreviewRuntime,
} from '../../../components/scene/hangar-preview-runtime';
import { pilotProfileForId } from '../../../components/scene/pilot-profile';
import { resolveWeaponLoadout } from '../../../components/scene/weapon-loadout';
import type { HangarDraft } from '../hangar-configuration';
import type { HangarPreviewStatus } from './hangar-preview';

interface HangarScenePreviewProps {
  draft: Readonly<HangarDraft>;
  resetVersion: number;
  onStatusChange: (status: HangarPreviewStatus) => void;
}

export function HangarScenePreview({ draft, resetVersion, onStatusChange }: HangarScenePreviewProps) {
  const mountRef = useRef<HTMLDivElement>(null);
  const runtimeRef = useRef<HangarPreviewRuntime | undefined>(undefined);
  const signatureRef = useRef('');
  const [error, setError] = useState<string>();
  const signature = `${draft.airframeId}:${draft.candidate}:${draft.pilotId}:${draft.effort}`;
  const configuration = useMemo<HangarPreviewConfiguration>(
    () => ({
      airframeId: draft.airframeId,
      candidate: draft.candidate,
      pilotProfile: pilotProfileForId(draft.pilotId),
      loadout: resolveWeaponLoadout(draft.effort),
    }),
    [draft.airframeId, draft.candidate, draft.pilotId, draft.effort]
  );

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return;
    onStatusChange('loading');
    try {
      runtimeRef.current = createHangarPreviewRuntime(mount, configuration);
      signatureRef.current = signature;
      setError(undefined);
      onStatusChange('ready');
    } catch (runtimeError) {
      setError(runtimeError instanceof Error ? runtimeError.message : 'WebGL preview unavailable');
      onStatusChange('unavailable');
    }
    return () => {
      runtimeRef.current?.dispose();
      runtimeRef.current = undefined;
      signatureRef.current = '';
    };
  }, []);

  useEffect(() => {
    if (!runtimeRef.current || signatureRef.current === signature) return;
    try {
      runtimeRef.current.setConfiguration(configuration);
      signatureRef.current = signature;
      setError(undefined);
      onStatusChange('ready');
    } catch (runtimeError) {
      setError(runtimeError instanceof Error ? runtimeError.message : 'Aircraft preview unavailable');
      onStatusChange('unavailable');
    }
  }, [configuration, onStatusChange, signature]);

  useEffect(() => {
    if (resetVersion > 0) runtimeRef.current?.reset();
  }, [resetVersion]);

  return (
    <div className='hangar-scene-preview'>
      <div className='hangar-scene-preview__mount' ref={mountRef} />
      {error ? <span className='hangar-scene-preview__error'>{error}</span> : null}
    </div>
  );
}
