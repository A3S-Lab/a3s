import { ImageResponse } from 'next/og';

export const alt = 'A3S — Governed agents, local AI work, and composable infrastructure';
export const size = { width: 1200, height: 630 };
export const contentType = 'image/png';
export const dynamic = 'force-static';

export default function OpenGraphImage() {
  return new ImageResponse(
    (
      <div
        style={{
          width: '100%',
          height: '100%',
          display: 'flex',
          position: 'relative',
          overflow: 'hidden',
          background: 'linear-gradient(125deg, #07101f 0%, #10152c 58%, #24163f 100%)',
          color: '#f5f7fb',
          fontFamily: 'Arial, sans-serif',
        }}
      >
        <div
          style={{
            position: 'absolute',
            inset: 0,
            opacity: 0.18,
            backgroundImage:
              'linear-gradient(rgba(145, 171, 220, 0.24) 1px, transparent 1px), linear-gradient(90deg, rgba(145, 171, 220, 0.24) 1px, transparent 1px)',
            backgroundSize: '58px 58px',
          }}
        />
        <div
          style={{
            position: 'absolute',
            width: 560,
            height: 560,
            right: -150,
            bottom: -250,
            border: '2px solid rgba(127, 161, 255, 0.24)',
            borderRadius: '50%',
            boxShadow: '0 0 0 70px rgba(127, 161, 255, 0.035), 0 0 0 140px rgba(127, 161, 255, 0.02)',
          }}
        />
        <div style={{ display: 'flex', width: '100%', padding: '62px 70px', flexDirection: 'column' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 17 }}>
            <div
              style={{
                display: 'flex',
                width: 54,
                height: 54,
                border: '3px solid #6ca3ff',
                borderRadius: '50%',
                alignItems: 'center',
                justifyContent: 'center',
                color: '#ffffff',
                fontSize: 28,
                fontWeight: 800,
              }}
            >
              A
            </div>
            <div style={{ display: 'flex', fontSize: 31, fontWeight: 800, letterSpacing: '-0.04em' }}>A3S</div>
            <div style={{ display: 'flex', color: '#78859a', fontSize: 13, fontWeight: 700, letterSpacing: '0.18em' }}>RUST-NATIVE AGENT PLATFORM</div>
          </div>

          <div style={{ display: 'flex', marginTop: 75, flexDirection: 'column' }}>
            <div style={{ display: 'flex', fontSize: 75, fontWeight: 800, letterSpacing: '-0.055em', lineHeight: 0.96 }}>One command.</div>
            <div style={{ display: 'flex', marginTop: 8, fontSize: 75, fontWeight: 800, letterSpacing: '-0.055em', lineHeight: 0.96 }}>Every boundary, explicit.</div>
            <div style={{ display: 'flex', width: 710, marginTop: 28, color: '#a7b2c3', fontSize: 24, lineHeight: 1.45 }}>
              Governed agents, local AI work, and composable infrastructure.
            </div>
          </div>

          <div style={{ display: 'flex', marginTop: 'auto', alignItems: 'center', gap: 14 }}>
            <div style={{ display: 'flex', padding: '14px 20px', border: '1px solid rgba(123, 160, 222, 0.35)', borderRadius: 8, background: 'rgba(4, 8, 16, 0.55)', color: '#dce7f8', fontFamily: 'monospace', fontSize: 17 }}>
              <span style={{ color: '#54dda1', marginRight: 12 }}>$</span> a3s code
            </div>
            {['CODE', 'WEB + WORK', 'RESEARCH', 'BOX'].map((item) => (
              <div key={item} style={{ display: 'flex', color: '#7e8a9d', fontSize: 12, fontWeight: 700, letterSpacing: '0.1em' }}>{item}</div>
            ))}
          </div>
        </div>
      </div>
    ),
    size,
  );
}
