'use client';

import type { ReactNode } from 'react';
import { Grid } from '@/components/canvas-ui/grid';

export function CanvasHero({ children }: { children: ReactNode }) {
  return (
    <Grid
      tileSize={112}
      gap={1}
      cornerRadius={8}
      amplitude={2.2}
      waveSpeed={0.52}
      frequency={13}
      waveWidth={0.075}
      fadeTime={0.7}
      maxLift={0.9}
      jitter={0.12}
      liftHeight={54}
      perspective={1180}
      tilt={0.42}
      shading={0.6}
      tint={[0.15, 0.39, 0.96]}
      tintStrength={0.48}
      idleRipples={3.4}
      className="a3s-canvas-hero"
    >
      {children}
    </Grid>
  );
}
