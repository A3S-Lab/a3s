import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { AircraftHud, type AircraftHudProps } from './aircraft-hud';

afterEach(cleanup);

const completeHud: AircraftHudProps = {
  open: true,
  id: 'j35-hud',
  anchor: { x: 144, y: 96 },
  placement: 'left',
  accentColor: '#2864e8',
  aircraft: {
    model: 'J-35',
    airframe: 'J35-01',
    manufacturer: 'Shenyang Aircraft Corporation',
  },
  agent: {
    name: 'A3S Code',
    pilot: 'A3S Code',
    livery: 'A3S Spectrum',
  },
  effort: {
    label: 'High',
    detail: 'Deep reasoning',
  },
  loadout: ['Repository context', 'Tool access'],
  task: {
    id: 'quick_file_edit',
    label: 'Quick file edit',
  },
  status: {
    label: 'RUNNING',
    tone: 'running',
    detail: 'Candidate execution · 02:14',
  },
  telemetry: [
    { label: 'MACH', value: '0.82', tone: 'accent' },
    { label: 'AOA', value: '+4.0', unit: 'deg' },
    { label: 'LOAD', value: '3.6', unit: 'G', tone: 'positive' },
  ],
};

describe('AircraftHud', () => {
  it('does not mount hidden hover content', () => {
    render(<AircraftHud {...completeHud} open={false} />);

    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });

  it('renders aircraft, agent, mission, status, and telemetry data', () => {
    render(<AircraftHud {...completeHud} />);

    const hud = screen.getByRole('tooltip', { name: 'A3S Code J-35 飞行信息' });
    expect(hud).toHaveAttribute('id', 'j35-hud');
    expect(hud).toHaveAttribute('data-placement', 'left');
    expect(hud).toHaveAttribute('data-status-tone', 'running');
    expect(hud).toHaveStyle({ left: '144px', top: '96px' });
    expect(screen.getByText('J35-01')).toBeInTheDocument();
    expect(screen.getByText('A3S Code · A3S Spectrum')).toBeInTheDocument();
    expect(screen.getByText('Deep reasoning')).toBeInTheDocument();
    expect(screen.getByText('Repository context')).toBeInTheDocument();
    expect(screen.getByText('quick_file_edit')).toBeInTheDocument();
    expect(screen.getByText('Candidate execution · 02:14')).toBeInTheDocument();
    expect(screen.getByText('0.82')).toBeInTheDocument();
    expect(screen.getByText('deg')).toBeInTheDocument();
  });

  it('keeps optional mission and telemetry regions out of a compact HUD', () => {
    render(
      <AircraftHud
        open
        aircraft={{ model: 'F-35 Lightning II' }}
        agent={{ name: 'Codex' }}
        status={{ label: 'READY', tone: 'ready' }}
        ariaLabel='Codex aircraft status'
      />
    );

    expect(screen.getByRole('tooltip', { name: 'Codex aircraft status' })).toBeInTheDocument();
    expect(screen.queryByRole('region', { name: '推理强度与挂载' })).not.toBeInTheDocument();
    expect(screen.queryByRole('definition')).not.toBeInTheDocument();
  });
});
