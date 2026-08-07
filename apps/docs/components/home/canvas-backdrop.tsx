'use client';

import { Grid } from '@/components/canvas-ui/grid';

export function CanvasBackdrop() {
  return (
    <Grid
      tileSize={72}
      gap={1}
      cornerRadius={5}
      amplitude={1.05}
      waveSpeed={0.48}
      frequency={12}
      waveWidth={0.07}
      fadeTime={0.78}
      maxLift={0.42}
      jitter={0.06}
      liftHeight={24}
      perspective={1280}
      tilt={0.24}
      shading={0.32}
      tint={[0.07, 0.39, 1]}
      tintStrength={0.18}
      idleRipples={0}
      pointerTarget="window"
      className="a3s-canvas-backdrop"
      style={{ position: 'fixed', inset: 0 }}
    >
      <div className="a3s-canvas-backdrop__surface" />
    </Grid>
  );
}
