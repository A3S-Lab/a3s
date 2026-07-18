import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { BenchController } from '../features/bench/use-bench-controller';
import { defaultTunnelParameters, labState } from '../state/lab-state';
import { TelemetryPanel } from './telemetry-panel';

describe('TelemetryPanel flight console', () => {
  beforeEach(() => {
    Object.assign(labState.tunnel, defaultTunnelParameters);
    labState.catalog.selectedTaskId = 'rust_multicrate_reconstruction';
  });

  afterEach(cleanup);

  it('presents map weather and keeps precise controls behind manual trim', () => {
    render(<TelemetryPanel actions={controllerStub()} />);

    const consoleSummary = screen.getByLabelText('展开飞行控制台');
    expect(consoleSummary.closest('details')).not.toHaveAttribute('open');
    expect(screen.getAllByText('冰雹')).toHaveLength(2);
    fireEvent.click(consoleSummary);
    expect(screen.getByText('飞行控制台')).toBeInTheDocument();
    const trimSummary = screen.getByText('手动飞控微调').closest('summary');
    expect(trimSummary?.parentElement).not.toHaveAttribute('open');
  });

  it('applies a flight profile through the existing tunnel controller API', () => {
    const actions = controllerStub();
    render(<TelemetryPanel actions={actions} />);

    fireEvent.click(screen.getByLabelText('展开飞行控制台'));
    fireEvent.click(screen.getByRole('button', { name: /截击/ }));

    expect(actions.setTunnelParameter).toHaveBeenNthCalledWith(1, 'mach', 1.45);
    expect(actions.setTunnelParameter).toHaveBeenNthCalledWith(2, 'angleOfAttack', 8);
    expect(actions.setTunnelParameter).toHaveBeenNthCalledWith(3, 'turbulence', 0.22);
  });
});

function controllerStub(): BenchController {
  return {
    refresh: vi.fn().mockResolvedValue(undefined),
    selectTask: vi.fn().mockResolvedValue(undefined),
    setQuery: vi.fn(),
    setCategory: vi.fn(),
    setIncludeBlocked: vi.fn(),
    setCandidate: vi.fn(),
    setCandidateLock: vi.fn(),
    setModel: vi.fn(),
    setEffort: vi.fn(),
    setTaskLock: vi.fn(),
    setLocked: vi.fn(),
    setDeploymentScope: vi.fn(),
    setTunnelParameter: vi.fn() as BenchController['setTunnelParameter'],
    resetTunnel: vi.fn(),
    startRun: vi.fn().mockResolvedValue(undefined),
    startCampaign: vi.fn().mockResolvedValue(true),
    stopCampaignTracking: vi.fn().mockReturnValue(true),
    dismissNotice: vi.fn(),
  };
}
