import type { HTMLAttributes, ReactNode } from 'react';

interface StatusBadgeProps extends Omit<HTMLAttributes<HTMLSpanElement>, 'children'> {
  tone?: 'neutral' | 'info' | 'success' | 'warning' | 'danger';
  children: ReactNode;
}

export function StatusBadge({ tone = 'neutral', children, className = '', ...props }: StatusBadgeProps) {
  return (
    <span {...props} className={`ds-status-badge ${tone}${className ? ` ${className}` : ''}`}>
      {children}
    </span>
  );
}
