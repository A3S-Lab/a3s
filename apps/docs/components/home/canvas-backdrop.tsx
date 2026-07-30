'use client';

import { Grid } from '@/components/canvas-ui/grid';

export function CanvasBackdrop() {
  return (
    <Grid
      tileSize={72}
      gap={1}
      cornerRadius={5}
      amplitude={1.4}
      waveSpeed={0.48}
      frequency={12}
      waveWidth={0.07}
      fadeTime={0.78}
      maxLift={0.6}
      jitter={0.06}
      liftHeight={32}
      perspective={1280}
      tilt={0.24}
      shading={0.48}
      tint={[0.15, 0.39, 0.96]}
      tintStrength={0.28}
      idleRipples={0}
      pointerTarget="window"
      className="a3s-canvas-backdrop"
      style={{ position: 'fixed', inset: 0 }}
    >
      <div className="a3s-canvas-backdrop__surface" />
    </Grid>
  );
}
