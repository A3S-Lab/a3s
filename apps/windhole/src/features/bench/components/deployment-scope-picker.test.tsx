import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DeploymentScopePicker } from './deployment-scope-picker';

describe('DeploymentScopePicker', () => {
  afterEach(cleanup);

  it('switches between the single-aircraft lead and the full roster with aria-pressed state', () => {
    const onChange = vi.fn();
    render(<DeploymentScopePicker scope='single' rosterSize={3} locked={false} onChange={onChange} />);

    const single = screen.getByRole('button', { name: /\u5355\u673a\u5148\u950b/ });
    const campaign = screen.getByRole('button', { name: /\u5168\u7f16\u961f \u00b7 3/ });
    expect(single).toHaveAttribute('aria-pressed', 'true');
    expect(campaign).toHaveAttribute('aria-pressed', 'false');
    expect(campaign).toBeEnabled();

    fireEvent.click(campaign);
    expect(onChange).toHaveBeenCalledOnce();
    expect(onChange).toHaveBeenCalledWith('campaign');
  });

  it('disables the campaign scope in locked mode without disabling the single-aircraft scope', () => {
    const onChange = vi.fn();
    render(<DeploymentScopePicker scope='single' rosterSize={5} locked={true} onChange={onChange} />);

    expect(screen.getByRole('button', { name: /\u5355\u673a\u5148\u950b/ })).toBeEnabled();
    const campaign = screen.getByRole('button', { name: /\u5168\u7f16\u961f \u00b7 5/ });
    expect(campaign).toBeDisabled();
    expect(campaign).toHaveAccessibleDescription(
      '\u9501\u6587\u4ef6\u6a21\u5f0f\u4ec5\u652f\u6301\u5355\u673a\u51fa\u51fb\u3002'
    );

    fireEvent.click(campaign);
    expect(onChange).not.toHaveBeenCalled();
  });

  it('does not offer an empty roster as a campaign deployment', () => {
    render(<DeploymentScopePicker scope='single' rosterSize={0} locked={false} onChange={vi.fn()} />);

    const campaign = screen.getByRole('button', { name: /\u5168\u7f16\u961f \u00b7 0/ });
    expect(campaign).toBeDisabled();
    expect(campaign).toHaveAccessibleDescription('\u8bf7\u5148\u5728\u673a\u5e93\u7ec4\u5efa\u7f16\u961f\u3002');
  });
});
