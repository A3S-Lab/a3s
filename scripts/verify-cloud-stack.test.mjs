import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  packageFromLock,
  parseCloudStackLock,
  tomlDependency,
  verifyCloudStack,
} from './verify-cloud-stack.mjs';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const LOCK_PATH = resolve(ROOT, 'compat/cloud-stack.acl');
const LOCK_SOURCE = readFileSync(LOCK_PATH, 'utf8');

test('the checked-in Cloud stack is reproducible and clean', () => {
  const result = verifyCloudStack(ROOT);
  assert.match(result.digest, /^sha256:[0-9a-f]{64}$/);
  assert.equal(result.components.length, 11);
  assert.deepEqual(result.aclFiles, ['config/cloud.acl', 'config/node.example.acl']);
});

test('the lock rejects unknown fields before accepting canonical text', () => {
  const mutated = LOCK_SOURCE.replace('  format = 1\n', '  format = 1\n  mystery = true\n');
  assert.throws(() => parseCloudStackLock(mutated), /unknown attribute mystery/);
});

test('the lock rejects unsafe and duplicate component paths', () => {
  const unsafe = LOCK_SOURCE.replace('  path = "crates/acl"\n', '  path = "../acl"\n');
  assert.throws(() => parseCloudStackLock(unsafe), /normalized repository-relative path/);

  const unsafeManifest = LOCK_SOURCE.replace(
    '  manifest = "src/runtime/Cargo.toml"\n',
    '  manifest = "../runtime/Cargo.toml"\n',
  );
  assert.throws(
    () => parseCloudStackLock(unsafeManifest),
    /component "box" manifest must be a normalized repository-relative path/,
  );

  const duplicate = LOCK_SOURCE.replace('  path = "crates/boot"\n', '  path = "crates/acl"\n');
  assert.throws(() => parseCloudStackLock(duplicate), /duplicate component path crates\/acl/);
});

test('the Box component resolves its package from the nested Rust workspace', () => {
  const box = parseCloudStackLock(LOCK_SOURCE).components.find(
    (component) => component.id === 'box',
  );
  assert.ok(box);
  assert.equal(box.manifest, 'src/runtime/Cargo.toml');
  assert.equal(box.package, 'a3s-box-runtime');
  assert.equal(box.version, '3.2.0');
});

test('Git lock selection ignores a same-version registry package', () => {
  const revision = '1675376dbced0a65afce2141e799dcdceb8e475d';
  const lock = `
[[package]]
name = "a3s-flow"
version = "0.4.3"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "a3s-flow"
version = "0.4.3"
source = "git+https://github.com/A3S-Lab/Flow.git?rev=${revision}#${revision}"
`;

  assert.equal(
    packageFromLock(
      lock,
      'a3s-flow',
      '0.4.3',
      'fixture lock',
      revision,
    ).source,
    `git+https://github.com/A3S-Lab/Flow.git?rev=${revision}#${revision}`,
  );
});

test('the lock must use canonical a3s-acl attribute ordering', () => {
  const noncanonical = LOCK_SOURCE.replace(
    '  owner = "A3S-Lab/ACL"\n  package = "a3s-acl"\n',
    '  package = "a3s-acl"\n  owner = "A3S-Lab/ACL"\n',
  );
  assert.throws(() => parseCloudStackLock(noncanonical), /not in canonical a3s-acl form/);
});

test('multiline Cargo dependency declarations are read as one binding', () => {
  const cloudManifest = readFileSync(resolve(ROOT, 'apps/cloud/Cargo.toml'), 'utf8');
  const boot = tomlDependency(
    cloudManifest,
    'workspace.dependencies',
    'a3s-boot',
    'apps/cloud/Cargo.toml',
  );
  assert.match(boot, /version = "=0\.1\.4"/);
  assert.match(boot, /"openapi-schemas"/);
});

test('the Cloud Runtime dependency is bound to the locked Git revision', () => {
  const cloudManifest = readFileSync(resolve(ROOT, 'apps/cloud/Cargo.toml'), 'utf8');
  const runtime = parseCloudStackLock(LOCK_SOURCE).components.find(
    (component) => component.id === 'runtime',
  );
  assert.ok(runtime);
  const declaration = tomlDependency(
    cloudManifest,
    'workspace.dependencies',
    runtime.package,
    'apps/cloud/Cargo.toml',
  );
  assert.ok(declaration.includes(`version = "=${runtime.version}"`));
  assert.ok(declaration.includes(`rev = "${runtime.revision}"`));
  assert.match(declaration, /git = "https:\/\/github\.com\/A3S-Lab\/Runtime\.git"/);
});

test('the Cloud Box Runtime dependency is bound to the locked Git revision', () => {
  const cloudManifest = readFileSync(resolve(ROOT, 'apps/cloud/Cargo.toml'), 'utf8');
  const box = parseCloudStackLock(LOCK_SOURCE).components.find(
    (component) => component.id === 'box',
  );
  assert.ok(box);
  const declaration = tomlDependency(
    cloudManifest,
    'workspace.dependencies',
    box.package,
    'apps/cloud/Cargo.toml',
  );
  assert.ok(declaration.includes(`version = "=${box.version}"`));
  assert.ok(declaration.includes(`rev = "${box.revision}"`));
  assert.match(declaration, /git = "https:\/\/github\.com\/A3S-Lab\/Box\.git"/);
});
