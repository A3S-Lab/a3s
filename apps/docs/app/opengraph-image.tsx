import { ImageResponse } from 'next/og';

export const alt = 'A3S — Agent tools, workflows, and runtimes';
export const size = { width: 1200, height: 630 };
export const contentType = 'image/png';
export const dynamic = 'force-static';

const orbitNodes = [
  { label: 'CLI', x: 250, y: 44 },
  { label: 'WEB', x: 396, y: 104 },
  { label: 'CLOUD', x: 456, y: 250 },
  { label: 'RUNTIME', x: 396, y: 396 },
  { label: 'BOX', x: 250, y: 456 },
  { label: 'FLOW', x: 104, y: 396 },
  { label: 'USE', x: 44, y: 250 },
  { label: 'CODE', x: 104, y: 104 },
] as const;

export default function OpenGraphImage() {
  return new ImageResponse(
    (
      <div
        style={{
          display: 'flex',
          width: '100%',
          height: '100%',
          overflow: 'hidden',
          background: '#000',
          color: '#e8eae8',
          fontFamily: 'monospace',
          flexDirection: 'column',
        }}
      >
        <div
          style={{
            display: 'flex',
            height: 62,
            padding: '0 34px',
            borderBottom: '1px solid #292d2c',
            alignItems: 'center',
          }}
        >
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              fontSize: 23,
              fontWeight: 700,
              letterSpacing: '-0.04em',
            }}
          >
            A3S
          </div>
          <div
            style={{
              display: 'flex',
              marginLeft: 15,
              color: '#606563',
              fontSize: 10,
              letterSpacing: '0.16em',
            }}
          >
            LAB
          </div>
          <div
            style={{
              display: 'flex',
              marginLeft: 'auto',
              color: '#777d7a',
              fontSize: 10,
              letterSpacing: '0.14em',
            }}
          >
            CODE / RUNTIME / CLOUD / TOOLS
          </div>
          <div
            style={{
              display: 'flex',
              marginLeft: 30,
              color: '#75c7c3',
              fontSize: 10,
              letterSpacing: '0.14em',
            }}
          >
            OPEN SOURCE
          </div>
        </div>

        <div style={{ display: 'flex', height: 568 }}>
          <div
            style={{
              display: 'flex',
              width: 610,
              padding: '58px 42px 34px',
              borderRight: '1px solid #666b69',
              flexDirection: 'column',
            }}
          >
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                color: '#9a9f9d',
                fontSize: 11,
                letterSpacing: '0.14em',
              }}
            >
              <span
                style={{
                  display: 'flex',
                  width: 9,
                  height: 9,
                  marginRight: 11,
                  border: '1px solid #75c7c3',
                  borderRadius: 9,
                }}
              >
                {' '}
              </span>
              RUST NATIVE · LOCAL FIRST
            </div>
            <div
              style={{
                display: 'flex',
                marginTop: 38,
                fontSize: 59,
                fontWeight: 500,
                letterSpacing: '-0.055em',
                lineHeight: 1.03,
                flexDirection: 'column',
              }}
            >
              <span style={{ display: 'flex' }}>Run Code locally.</span>
              <span style={{ display: 'flex', marginTop: 4 }}>
                Add what the job
              </span>
              <span style={{ display: 'flex', color: '#75c7c3' }}>
                actually needs.
              </span>
            </div>
            <div
              style={{
                display: 'flex',
                width: 510,
                marginTop: 28,
                color: '#9a9f9d',
                fontSize: 17,
                lineHeight: 1.55,
              }}
            >
              34 independently released projects, each mapped from its own
              source and architecture notes.
            </div>
            <div
              style={{
                display: 'flex',
                width: 230,
                height: 42,
                marginTop: 'auto',
                padding: '0 14px',
                borderTop: '1px solid #292d2c',
                borderBottom: '1px solid #292d2c',
                alignItems: 'center',
                color: '#d5d8d5',
                fontSize: 14,
              }}
            >
              <span
                style={{ display: 'flex', marginRight: 12, color: '#75c7c3' }}
              >
                $
              </span>
              a3s code
            </div>
          </div>

          <div
            style={{
              display: 'flex',
              width: 590,
              padding: '26px 30px 20px',
              flexDirection: 'column',
            }}
          >
            <div
              style={{
                display: 'flex',
                height: 34,
                borderBottom: '1px solid #292d2c',
                alignItems: 'center',
                color: '#777d7a',
                fontSize: 10,
                letterSpacing: '0.12em',
              }}
            >
              <span style={{ display: 'flex', gap: 4 }}>
                {[0, 1, 2].map((bar) => (
                  <span
                    key={bar}
                    style={{
                      display: 'flex',
                      width: 3,
                      height: 12,
                      background: '#e8eae8',
                    }}
                  />
                ))}
              </span>
              <span style={{ display: 'flex', marginLeft: 'auto' }}>
                A3S / PROJECT MAP
              </span>
            </div>
            <div
              style={{
                display: 'flex',
                width: 500,
                height: 500,
                margin: '3px auto 0',
                position: 'relative',
              }}
            >
              <svg width="500" height="500" viewBox="0 0 500 500">
                <circle
                  cx="250"
                  cy="250"
                  r="206"
                  fill="none"
                  stroke="#a7aaa8"
                  strokeWidth="1"
                />
                {orbitNodes.map((node) => (
                  <line
                    key={`line-${node.label}`}
                    x1="250"
                    x2={node.x}
                    y1="250"
                    y2={node.y}
                    stroke="#505452"
                    strokeDasharray="3 4"
                    strokeWidth="1"
                  />
                ))}
                <path
                  d="M104 104 C104 185 186 165 186 250 S104 315 104 396"
                  fill="none"
                  stroke="#8a8f8c"
                  strokeDasharray="3 4"
                  strokeWidth="1"
                />
                <path
                  d="M396 104 C396 185 314 165 314 250 S396 315 396 396"
                  fill="none"
                  stroke="#8a8f8c"
                  strokeDasharray="3 4"
                  strokeWidth="1"
                />
                <path
                  d="M44 250 C140 250 150 185 250 185 S360 250 456 250"
                  fill="none"
                  stroke="#8a8f8c"
                  strokeDasharray="3 4"
                  strokeWidth="1"
                />
                <path
                  d="M44 250 C140 250 150 315 250 315 S360 250 456 250"
                  fill="none"
                  stroke="#8a8f8c"
                  strokeDasharray="3 4"
                  strokeWidth="1"
                />
                {orbitNodes.map((node) => (
                  <circle
                    key={node.label}
                    cx={node.x}
                    cy={node.y}
                    r="6"
                    fill="#000"
                    stroke="#b5b9b7"
                    strokeWidth="1"
                  />
                ))}
              </svg>
              <div
                style={{
                  display: 'flex',
                  position: 'absolute',
                  top: 219,
                  left: 174,
                  width: 152,
                  height: 62,
                  border: '1px dashed #7f8481',
                  background: '#000',
                  alignItems: 'center',
                  justifyContent: 'center',
                  flexDirection: 'column',
                }}
              >
                <span
                  style={{
                    display: 'flex',
                    color: '#9a9f9d',
                    fontSize: 10,
                    letterSpacing: '0.06em',
                  }}
                >
                  shared runtime
                </span>
                <span
                  style={{
                    display: 'flex',
                    marginTop: 6,
                    color: '#e8eae8',
                    fontSize: 13,
                  }}
                >
                  AgentSession
                </span>
              </div>
              {orbitNodes.map((node) => (
                <div
                  key={node.label}
                  style={{
                    display: 'flex',
                    position: 'absolute',
                    top: node.y < 250 ? node.y - 25 : node.y + 10,
                    left: node.x - 45,
                    width: 90,
                    justifyContent: 'center',
                    color: '#b8bcba',
                    fontSize: 9,
                    letterSpacing: '0.05em',
                  }}
                >
                  {node.label}
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    ),
    size,
  );
}
