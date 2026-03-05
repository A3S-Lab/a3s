#!/usr/bin/env node

import { mkdirSync, writeFileSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const projectRoot = join(__dirname, '..');
const sidecarDir = join(projectRoot, 'src-tauri', 'sidecar');

// Get target triple for platform-specific binary naming
function getTargetTriple() {
    const platform = process.platform;
    const arch = process.arch;

    if (platform === 'win32') {
        return arch === 'x64' ? 'x86_64-pc-windows-msvc' : 'aarch64-pc-windows-msvc';
    } else if (platform === 'darwin') {
        return arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin';
    } else if (platform === 'linux') {
        return arch === 'x64' ? 'x86_64-unknown-linux-gnu' : 'aarch64-unknown-linux-gnu';
    }

    throw new Error(`Unsupported platform: ${platform}-${arch}`);
}

const targetTriple = getTargetTriple();
const binaryName = process.platform === 'win32'
    ? `studio-sidecar-${targetTriple}.exe`
    : `studio-sidecar-${targetTriple}`;

const placeholderPath = join(sidecarDir, binaryName);

// Create sidecar directory if it doesn't exist
mkdirSync(sidecarDir, { recursive: true });

// Create placeholder binary if it doesn't exist
if (!existsSync(placeholderPath)) {
    console.log(`📦 Creating placeholder sidecar: ${binaryName}`);

    // Create a minimal executable placeholder
    const placeholderContent = process.platform === 'win32'
        ? Buffer.from([0x4D, 0x5A]) // MZ header for Windows
        : Buffer.from([0x7F, 0x45, 0x4C, 0x46]); // ELF header for Unix

    writeFileSync(placeholderPath, placeholderContent);
    console.log('✅ Placeholder created');
} else {
    console.log('✅ Sidecar binary already exists');
}
