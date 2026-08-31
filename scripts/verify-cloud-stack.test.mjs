import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
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
const CLOUD_STACK_WORKFLOW_SOURCE = readFileSync(
  resolve(ROOT, '.github/workflows/cloud-stack.yml'),
  'utf8',
);

function committedFile(repository, path) {
  return execFileSync('git', ['show', `HEAD:${path}`], {
    cwd: resolve(ROOT, repository),
    encoding: null,
  });
}

test('the checked-in Cloud stack is reproducible and clean', () => {
  const result = verifyCloudStack(ROOT);
  assert.match(result.digest, /^sha256:[0-9a-f]{64}$/);
  assert.equal(result.components.length, 14);
  assert.deepEqual(result.aclFiles, ['config/cloud.acl', 'config/node.example.acl']);
  assert.match(result.formInteractionFixture.digest, /^sha256:[0-9a-f]{64}$/);
  assert.equal(
    result.formInteractionFixture.owner,
    'packages/ui/modules/form/tests/conformance/interaction-contract-v1.json',
  );
  assert.equal(
    result.formInteractionFixture.cloud,
    'apps/cloud/crates/control-plane/tests/fixtures/form-interaction-contract-v1.json',
  );
  assert.match(result.formValueEvaluationFixture.digest, /^sha256:[0-9a-f]{64}$/);
  assert.equal(
    result.formValueEvaluationFixture.owner,
    'packages/ui/modules/form/tests/conformance/value-evaluation-v1.json',
  );
  assert.equal(
    result.formValueEvaluationFixture.cloud,
    'apps/cloud/crates/control-plane/tests/fixtures/form-value-evaluation-v1.json',
  );
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

test('the Form component resolves the native core package from its nested manifest', () => {
  const form = parseCloudStackLock(LOCK_SOURCE).components.find(
    (component) => component.id === 'form',
  );
  assert.ok(form);
  assert.equal(form.manifest, 'modules/form/crates/a3s-form-core/Cargo.toml');
  assert.equal(form.owner, 'A3S-Lab/UI');
  assert.equal(form.path, 'packages/ui');
  assert.equal(form.repository, 'git@github.com:A3S-Lab/UI.git');
  assert.equal(form.package, 'a3s-form-core');
  assert.equal(form.version, '0.1.0');
});

test('the Code component describes the Cloud host dependency', () => {
  const code = parseCloudStackLock(LOCK_SOURCE).components.find(
    (component) => component.id === 'code',
  );
  assert.ok(code);
  assert.equal(code.dependencySource, 'git');
  assert.equal(code.manifest, 'core/Cargo.toml');
  assert.equal(code.package, 'a3s-code-core');
  assert.equal(code.source, 'git');
  assert.equal(code.version, '8.0.4');
});

test('published dependencies retain separate Git provenance', () => {
  const components = new Map(
    parseCloudStackLock(LOCK_SOURCE).components.map((component) => [
      component.id,
      component,
    ]),
  );

  for (const id of ['flow']) {
    const component = components.get(id);
    assert.ok(component);
    assert.equal(component.dependencySource, 'registry');
    assert.equal(component.source, 'git');
    assert.match(component.revision, /^[0-9a-f]{40}$/);
  }
});

test('Cloud consumes Flow through an exact published release', () => {
  const cloudManifest = readFileSync(resolve(ROOT, 'apps/cloud/Cargo.toml'), 'utf8');
  const cloudLock = readFileSync(resolve(ROOT, 'apps/cloud/Cargo.lock'), 'utf8');
  const components = new Map(
    parseCloudStackLock(LOCK_SOURCE).components.map((component) => [component.id, component]),
  );

  for (const id of ['flow']) {
    const component = components.get(id);
    assert.ok(component);
    const declaration = tomlDependency(
      cloudManifest,
      'workspace.dependencies',
      component.package,
      'apps/cloud/Cargo.toml',
    );
    assert.ok(declaration.includes(`version = "=${component.version}"`));
    assert.doesNotMatch(declaration, /\b(?:git|rev|path|registry)\s*=/);

    const lockedPackage = packageFromLock(
      cloudLock,
      component.package,
      component.version,
      'apps/cloud/Cargo.lock',
      undefined,
      'registry+https://github.com/rust-lang/crates.io-index',
    );
    assert.match(lockedPackage.checksum, /^[0-9a-f]{64}$/);
  }
});

test('the Cloud Code dependency is bound to the locked Git revision', () => {
  const cloudManifest = readFileSync(resolve(ROOT, 'apps/cloud/Cargo.toml'), 'utf8');
  const code = parseCloudStackLock(LOCK_SOURCE).components.find(
    (component) => component.id === 'code',
  );
  assert.ok(code);
  const declaration = tomlDependency(
    cloudManifest,
    'workspace.dependencies',
    code.package,
    'apps/cloud/Cargo.toml',
  );
  assert.ok(declaration.includes(`version = "=${code.version}"`));
  assert.ok(declaration.includes(`rev = "${code.revision}"`));
  assert.match(declaration, /git = "https:\/\/github\.com\/A3S-Lab\/Code\.git"/);
});

test('component dependency sources reject unknown and workspace-registry combinations', () => {
  const unknown = LOCK_SOURCE.replace(
    '  dependency_source = "registry"\n',
    '  dependency_source = "mirror"\n',
  );
  assert.throws(
    () => parseCloudStackLock(unknown),
    /dependency_source must be git, registry, or workspace/,
  );

  const workspaceRegistry = LOCK_SOURCE.replace(
    'component "updater" {\n',
    'component "updater" {\n  dependency_source = "registry"\n',
  );
  assert.throws(
    () => parseCloudStackLock(workspaceRegistry),
    /workspace component "updater" dependency_source must be workspace/,
  );
});

test('the Use component describes the sole shared manager repository', () => {
  const use = parseCloudStackLock(LOCK_SOURCE).components.find(
    (component) => component.id === 'use',
  );
  assert.ok(use);
  assert.equal(use.manifest, 'Cargo.toml');
  assert.equal(use.package, 'a3s-use');
  assert.equal(use.version, '0.3.3');
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

test('protocol levels accept stable and prerelease owner schemas and reject version drift', () => {
  const slashVersion = LOCK_SOURCE.replace(
    'schema = "a3s.flow.native_ts.v1"',
    'schema = "a3s.dev/flow-native-typescript/v1"',
  );
  assert.doesNotThrow(() => parseCloudStackLock(slashVersion));

  const alphaVersion = slashVersion.replace(
    'schema = "a3s.dev/flow-native-typescript/v1"',
    'schema = "a3s.dev/flow-native-typescript/v1alpha1"',
  );
  assert.doesNotThrow(() => parseCloudStackLock(alphaVersion));

  const driftedVersion = alphaVersion.replace(
    'schema = "a3s.dev/flow-native-typescript/v1alpha1"',
    'schema = "a3s.dev/flow-native-typescript/v2alpha1"',
  );
  assert.throws(() => parseCloudStackLock(driftedVersion), /does not match level 1/);
});

test('the lock registers both owner-defined Form evaluation protocols', () => {
  const protocols = new Map(
    parseCloudStackLock(LOCK_SOURCE).protocols.map((protocol) => [protocol.id, protocol]),
  );
  assert.equal(
    protocols.get('form-core-evaluate-request')?.schema,
    'a3s.dev/form-core/evaluate-request/v1alpha1',
  );
  assert.equal(
    protocols.get('form-core-evaluate-response')?.schema,
    'a3s.dev/form-core/evaluate-response/v1alpha1',
  );
});

test('the lock registers the complete Use protocol-level-6 host boundary', () => {
  const protocols = new Map(
    parseCloudStackLock(LOCK_SOURCE).protocols.map((protocol) => [protocol.id, protocol]),
  );
  assert.equal(
    protocols.get('use-plugin-host-capabilities')?.schema,
    'a3s.use.plugin-host-capabilities.v6',
  );
  assert.equal(
    protocols.get('use-plugin-host-managed-scope')?.schema,
    'a3s.use.plugin-managed-scope.v2',
  );
  assert.equal(protocols.get('use-plugin-host-managed-scope')?.level, 2);
  for (const id of [
    'use-plugin-host-apply-request',
    'use-plugin-host-apply-result',
    'use-plugin-host-enablement-plan-request',
    'use-plugin-host-enablement-plan-result',
    'use-plugin-host-observation-request',
    'use-plugin-host-observation-result',
    'use-plugin-host-plan-request',
    'use-plugin-host-plan-result',
  ]) {
    assert.equal(protocols.get(id)?.level, 1, `${id} must remain protocol level 1`);
  }
});

test('the lock registers the Runtime lifecycle contract boundary', () => {
  const protocols = new Map(
    parseCloudStackLock(LOCK_SOURCE).protocols.map((protocol) => [protocol.id, protocol]),
  );
  assert.equal(
    protocols.get('runtime-capabilities')?.schema,
    'a3s.runtime.capabilities.v6',
  );
  assert.equal(protocols.get('runtime-capabilities')?.level, 6);
  assert.equal(
    protocols.get('runtime-observation')?.schema,
    'a3s.runtime.observation.v4',
  );
  assert.equal(protocols.get('runtime-observation')?.level, 4);
  assert.equal(protocols.get('runtime-unit-spec')?.schema, 'a3s.runtime.unit-spec.v4');
  assert.equal(protocols.get('runtime-unit-spec')?.level, 4);
});

test('multiline Cargo dependency declarations are read as one binding', () => {
  const cloudManifest = readFileSync(resolve(ROOT, 'apps/cloud/Cargo.toml'), 'utf8');
  const boot = tomlDependency(
    cloudManifest,
    'workspace.dependencies',
    'a3s-boot',
    'apps/cloud/Cargo.toml',
  );
  assert.match(boot, /version = "=0\.2\.0"/);
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

test('the Cloud Form Core dependency is exact and bound to the locked Git revision', () => {
  const cloudManifest = readFileSync(resolve(ROOT, 'apps/cloud/Cargo.toml'), 'utf8');
  const form = parseCloudStackLock(LOCK_SOURCE).components.find(
    (component) => component.id === 'form',
  );
  assert.ok(form);
  const declaration = tomlDependency(
    cloudManifest,
    'workspace.dependencies',
    form.package,
    'apps/cloud/Cargo.toml',
  );
  assert.ok(declaration.includes(`version = "=${form.version}"`));
  assert.ok(declaration.includes(`rev = "${form.revision}"`));
  assert.match(declaration, /git = "https:\/\/github\.com\/A3S-Lab\/UI\.git"/);
});

test('Cloud Stack CI watches and initializes every locked Git component', () => {
  const watchedPaths = new Set(
    CLOUD_STACK_WORKFLOW_SOURCE.split(/\r?\n/)
      .map((line) => /^\s*- "([^"]+)"$/.exec(line)?.[1])
      .filter(Boolean),
  );
  const initializedPaths = new Set(
    CLOUD_STACK_WORKFLOW_SOURCE.split(/\r?\n/).map((line) => {
      const value = line.trim();
      return value.endsWith('\\') ? value.slice(0, -1).trimEnd() : value;
    }),
  );
  const gitComponents = parseCloudStackLock(LOCK_SOURCE).components.filter(
    (component) => component.source === 'git',
  );
  for (const component of gitComponents) {
    assert.ok(
      watchedPaths.has(component.path),
      `${component.path} is not watched by Cloud Stack CI`,
    );
    assert.ok(
      initializedPaths.has(component.path),
      `${component.path} is not initialized by Cloud Stack CI`,
    );
  }
});

test('Cloud consumes the UI-owned Form interaction fixture byte for byte', () => {
  const owner = committedFile(
    'packages/ui',
    'modules/form/tests/conformance/interaction-contract-v1.json',
  );
  const cloud = committedFile(
    'apps/cloud',
    'crates/control-plane/tests/fixtures/form-interaction-contract-v1.json',
  );
  assert.ok(owner.equals(cloud));
});

test('Cloud consumes the UI-owned Form submitted-value evaluation fixture byte for byte', () => {
  const owner = committedFile(
    'packages/ui',
    'modules/form/tests/conformance/value-evaluation-v1.json',
  );
  const cloud = committedFile(
    'apps/cloud',
    'crates/control-plane/tests/fixtures/form-value-evaluation-v1.json',
  );
  assert.ok(owner.equals(cloud));
});
