import { withBase } from '@rspress/core/runtime';

export function A3SMark({ className }: { className?: string }) {
  return (
    <img
      alt=""
      aria-hidden="true"
      className={className}
      height="48"
      src={withBase('/brand/a3s-os-logo.png')}
      width="48"
    />
  );
}
