import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const baseStyles = readFileSync(resolve(process.cwd(), 'src/styles/base.css'), 'utf8');

describe('base color contrast', () => {
  it('keeps quiet dark-theme text readable against the main panel', () => {
    const darkTheme = baseStyles.match(/:root\.dark\s*\{(?<tokens>[\s\S]*?)\n\}/)?.groups?.tokens;

    expect(darkTheme).toBeDefined();
    expect(
      contrastRatio(readHexToken(darkTheme!, '--a3s-faint'), readHexToken(darkTheme!, '--a3s-panel'))
    ).toBeGreaterThanOrEqual(4.5);
  });
});

function readHexToken(css: string, token: string): string {
  const escapedToken = token.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const value = css.match(new RegExp(`${escapedToken}:\\s*(#[0-9a-f]{6});`, 'i'))?.[1];
  if (!value) throw new Error(`Missing six-digit color token: ${token}`);
  return value;
}

function contrastRatio(left: string, right: string): number {
  const leftLuminance = relativeLuminance(left);
  const rightLuminance = relativeLuminance(right);
  return (Math.max(leftLuminance, rightLuminance) + 0.05) / (Math.min(leftLuminance, rightLuminance) + 0.05);
}

function relativeLuminance(color: string): number {
  const channels = color
    .slice(1)
    .match(/.{2}/g)
    ?.map((value) => Number.parseInt(value, 16) / 255)
    .map((value) => (value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4));
  if (!channels || channels.length !== 3) throw new Error(`Invalid RGB color: ${color}`);
  return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
}
