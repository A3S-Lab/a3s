import type { ReactNode } from 'react';

export type IconName =
  | 'code'
  | 'shield'
  | 'spark'
  | 'runtime'
  | 'history'
  | 'search'
  | 'check'
  | 'branch'
  | 'database';

export function CapabilityIcon({ name }: { name: IconName }) {
  let paths: ReactNode;

  switch (name) {
    case 'code':
      paths = (
        <>
          <path d="m8 9-4 3 4 3" />
          <path d="m16 9 4 3-4 3" />
          <path d="m14 5-4 14" />
        </>
      );
      break;
    case 'shield':
      paths = (
        <>
          <path d="M12 3 4.8 6v5.2c0 4.4 2.8 7.8 7.2 9.8 4.4-2 7.2-5.4 7.2-9.8V6L12 3Z" />
          <path d="M12 8v4" />
          <path d="M12 16h.01" />
        </>
      );
      break;
    case 'spark':
      paths = (
        <>
          <path d="m12 3-1 4-4 1 4 1 1 4 1-4 4-1-4-1-1-4Z" />
          <path d="m18 14-.7 2.3L15 17l2.3.7L18 20l.7-2.3L21 17l-2.3-.7L18 14Z" />
          <path d="m5 13-.6 1.9-1.9.6 1.9.6L5 18l.6-1.9 1.9-.6-1.9-.6L5 13Z" />
        </>
      );
      break;
    case 'runtime':
      paths = (
        <>
          <path d="M7.5 18.5H6a4 4 0 0 1-.4-8A6.5 6.5 0 0 1 18 8.5a4.5 4.5 0 0 1 .1 9H16" />
          <path d="M12 12v9" />
          <path d="m8.8 15.2 3.2-3.2 3.2 3.2" />
        </>
      );
      break;
    case 'history':
      paths = (
        <>
          <path d="M3 12a9 9 0 1 0 3-6.7L3 8" />
          <path d="M3 3v5h5" />
          <path d="M12 7v5l3 2" />
        </>
      );
      break;
    case 'search':
      paths = (
        <>
          <circle cx="11" cy="11" r="6" />
          <path d="m16 16 4 4" />
        </>
      );
      break;
    case 'check':
      paths = <path d="m5 12 4 4 10-10" />;
      break;
    case 'branch':
      paths = (
        <>
          <circle cx="6" cy="5" r="2" />
          <circle cx="18" cy="7" r="2" />
          <circle cx="6" cy="19" r="2" />
          <path d="M6 7v10M8 7c5 0 3 0 8 0" />
        </>
      );
      break;
    case 'database':
      paths = (
        <>
          <ellipse cx="12" cy="5" rx="7" ry="3" />
          <path d="M5 5v7c0 1.7 3.1 3 7 3s7-1.3 7-3V5" />
          <path d="M5 12v5c0 1.7 3.1 3 7 3s7-1.3 7-3v-5" />
        </>
      );
      break;
  }

  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      {paths}
    </svg>
  );
}
