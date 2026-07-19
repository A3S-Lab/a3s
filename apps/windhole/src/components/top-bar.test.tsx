import { cleanup, fireEvent, render, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { BenchController } from '../features/bench/use-bench-controller';
import { labState } from '../state/lab-state';
import { TopBar } from './top-bar';

const actions = {
  refresh: async () => undefined,
  selectTask: async () => undefined,
  setQuery: () => undefined,
  setCategory: () => undefined,
  setIncludeBlocked: () => undefined,
  setCandidate: () => undefined,
  setCandidateLock: () => undefined,
  setModel: () => undefined,
  setEffort: () => undefined,
  setTaskLock: () => undefined,
  setLocked: () => undefined,
  setDeploymentScope: () => undefined,
  setTunnelParameter: () => undefined,
  resetTunnel: () => undefined,
  startRun: async () => undefined,
  startCampaign: async () => true,
  stopCampaignTracking: () => true,
  dismissNotice: () => undefined,
} satisfies BenchController;

afterEach(() => {
  cleanup();
  labState.workspace = 'lab';
});

describe('TopBar', () => {
  it('renders the complete product brand once without legacy wind-tunnel copy', () => {
    const { container } = render(<TopBar actions={actions} />);
    const header = container.querySelector('.top-bar');
    const brand = container.querySelector('.brand-lockup');

    expect(header).toBeInstanceOf(HTMLElement);
    expect(brand).toBeInstanceOf(HTMLElement);

    const brandQueries = within(brand as HTMLElement);
    const brandTitle = brandQueries.getByText('A3S智能体评测', { exact: true });

    expect(brandTitle).toBeVisible();
    expect(brandTitle).toHaveClass('brand-title');
    expect(brandTitle.childNodes).toHaveLength(1);
    expect(brandTitle.firstChild?.nodeType).toBe(Node.TEXT_NODE);
    expect(brandQueries.queryByText('A3S', { exact: true })).not.toBeInTheDocument();
    expect(brandQueries.queryByText('智能体评测', { exact: true })).not.toBeInTheDocument();
    expect(header).not.toHaveTextContent(/wind\s*tunnel|风洞/i);
  });

  it('opens the formation hangar from the primary game navigation', () => {
    const { container } = render(<TopBar actions={actions} />);

    fireEvent.click(within(container).getByRole('button', { name: '机库编队' }));

    expect(labState.workspace).toBe('hangar');
  });
});
