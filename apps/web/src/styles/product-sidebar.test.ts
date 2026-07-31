import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const baseStyles = readFileSync(resolve(process.cwd(), 'src/styles/base.css'), 'utf8');
const sidebarStyles = readFileSync(resolve(process.cwd(), 'src/styles/product-sidebar.css'), 'utf8');
const activityBarStyles = readFileSync(resolve(process.cwd(), 'src/styles/activity-bar.css'), 'utf8');
const designSystemStyles = readFileSync(resolve(process.cwd(), 'src/styles/design-system.css'), 'utf8');
const memoryStyles = readFileSync(resolve(process.cwd(), 'src/styles/memory.css'), 'utf8');
const settingsStyles = readFileSync(resolve(process.cwd(), 'src/styles/settings.css'), 'utf8');

describe('shared product navigation styles', () => {
  it('uses shared shell dimensions and icon stroke tokens', () => {
    expect(baseStyles).toMatch(/--a3s-product-sidebar-width:\s*232px;/);
    expect(baseStyles).toMatch(/--a3s-product-sidebar-compact-width:\s*208px;/);
    expect(baseStyles).toMatch(/--a3s-shell-icon-stroke:\s*1\.75;/);
    expect(sidebarStyles).toMatch(
      /\.product-sidebar\s*\{[\s\S]*?width:\s*var\(--a3s-product-sidebar-width\);[\s\S]*?padding:\s*0 12px 14px;/
    );
  });

  it('frames navigation glyphs consistently and keeps semantic tones explicit', () => {
    expect(sidebarStyles).toMatch(
      /\.sidebar-nav-icon\s*\{[\s\S]*?width:\s*25px;[\s\S]*?height:\s*25px;[\s\S]*?border-radius:\s*8px;/
    );
    expect(sidebarStyles).toMatch(/\.sidebar-nav-icon\[data-tone="blue"\]/);
    expect(sidebarStyles).toMatch(/\.sidebar-nav-icon\[data-tone="green"\]/);
    expect(sidebarStyles).toMatch(/\.sidebar-nav-icon\[data-tone="orange"\]/);
  });

  it('aligns Memory and Market filter rails to the shared sidebar width', () => {
    expect(memoryStyles).toMatch(
      /\.memory-workbench\s*\{[\s\S]*?grid-template-columns:\s*var\(--a3s-product-sidebar-width\) minmax\(0, 1fr\);/
    );
  });

  it('uses the shared shell icon stroke in every top-level navigation surface', () => {
    expect(activityBarStyles).toMatch(
      /\.activity-button > svg\s*\{[\s\S]*?stroke-width:\s*var\(--a3s-shell-icon-stroke\);/
    );
    expect(designSystemStyles).toMatch(
      /\.ds-page-header \.lucide\s*\{[\s\S]*?stroke-width:\s*var\(--a3s-shell-icon-stroke\);/
    );
    expect(settingsStyles).toMatch(
      /\.settings-nav nav button svg\s*\{[\s\S]*?stroke-width:\s*var\(--a3s-shell-icon-stroke\);/
    );
  });

  it('shows readable product labels on desktop and returns to the compact rail on mobile', () => {
    expect(activityBarStyles).toMatch(/\.activity-bar\s*\{[\s\S]*?width:\s*76px;[\s\S]*?flex:\s*0 0 76px;/);
    expect(activityBarStyles).toMatch(/\.activity-button-label\s*\{[\s\S]*?font-size:\s*9px;/);
    expect(activityBarStyles).toMatch(
      /@media \(max-width: 620px\)[\s\S]*?\.activity-bar\s*\{[\s\S]*?width:\s*52px;[\s\S]*?\.activity-button-label\s*\{[\s\S]*?display:\s*none;/
    );
  });
});
