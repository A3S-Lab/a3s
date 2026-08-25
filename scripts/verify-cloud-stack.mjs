#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, isAbsolute, join, normalize, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const require = createRequire(import.meta.url);
const { generate, parse } = require('../crates/acl/sdk/node/src');

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const DEFAULT_ROOT = resolve(dirname(SCRIPT_PATH), '..');
const CRATES_IO_SOURCE = 'registry+https://github.com/rust-lang/crates.io-index';
const REQUIRED_COMPONENTS = [
  'acl',
  'boot',
  'box',
  'cloud',
  'code',
  'event',
  'flow',
  'form',
  'gateway',
  'orm',
  'runtime',
  'sentry',
  'updater',
  'use',
];

function invariant(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function attribute(block, name, kind) {
  const value = block.attributes.get(name);
  invariant(value, `${block.name} "${block.labels[0] ?? ''}" is missing ${name}`);
  invariant(
    value.kind === kind,
    `${block.name} "${block.labels[0] ?? ''}" attribute ${name} must be ${kind}`,
  );
  return value.value;
}

function optionalAttribute(block, name, kind) {
  const value = block.attributes.get(name);
  if (!value) {
    return undefined;
  }
  invariant(
    value.kind === kind,
    `${block.name} "${block.labels[0] ?? ''}" attribute ${name} must be ${kind}`,
  );
  return value.value;
}

function validateAttributes(block, required, optional = []) {
  invariant(block.labels.length === 1, `${block.name} blocks require exactly one label`);
  invariant(block.blocks.length === 0, `${block.name} "${block.labels[0]}" cannot contain blocks`);
  const allowed = new Set([...required, ...optional]);
  for (const key of block.attributes.keys()) {
    invariant(allowed.has(key), `${block.name} "${block.labels[0]}" has unknown attribute ${key}`);
  }
  for (const key of required) {
    invariant(
      block.attributes.has(key),
      `${block.name} "${block.labels[0]}" is missing required attribute ${key}`,
    );
  }
}

function validateRelativePath(value, label) {
  invariant(value.length > 0, `${label} cannot be empty`);
  invariant(!isAbsolute(value), `${label} must be relative`);
  invariant(!value.includes('\\') && !value.includes('\0'), `${label} uses unsafe separators`);
  invariant(
    normalize(value).split(sep).join('/') === value &&
      value !== '..' &&
      !value.startsWith('../'),
    `${label} must be a normalized repository-relative path`,
  );
}

function validateSorted(values, label) {
  const sorted = [...values].sort();
  invariant(
    values.every((value, index) => value === sorted[index]),
    `${label} must be sorted by label`,
  );
}

export function parseCloudStackLock(source) {
  let document;
  try {
    document = parse(source);
  } catch (error) {
    const location =
      Number.isInteger(error?.line) && Number.isInteger(error?.column)
        ? ` at ${error.line}:${error.column}`
        : '';
    throw new Error(`could not parse compatibility lock${location}: ${error?.message ?? error}`);
  }

  invariant(document.blocks.length > 0, 'compatibility lock cannot be empty');
  invariant(document.blocks[0].name === 'stack', 'the first block must be stack');
  const stackBlocks = document.blocks.filter((block) => block.name === 'stack');
  invariant(stackBlocks.length === 1, 'compatibility lock requires exactly one stack block');
  const stack = stackBlocks[0];
  validateAttributes(stack, ['format', 'owner', 'rust_version', 'schema']);
  invariant(stack.labels[0] === 'cloud', 'stack label must be "cloud"');
  invariant(attribute(stack, 'format', 'Number') === 1, 'stack format must be 1');
  invariant(
    attribute(stack, 'schema', 'String') === 'a3s.cloud.compatibility-lock.v1',
    'unsupported compatibility-lock schema',
  );
  invariant(attribute(stack, 'owner', 'String') === 'A3S-Lab/Cloud', 'unexpected stack owner');
  invariant(
    /^\d+\.\d+\.\d+$/.test(attribute(stack, 'rust_version', 'String')),
    'invalid Rust version',
  );

  const allowedBlocks = new Set(['stack', 'component', 'protocol']);
  for (const block of document.blocks) {
    invariant(allowedBlocks.has(block.name), `unknown top-level block ${block.name}`);
  }

  const componentBlocks = document.blocks.filter((block) => block.name === 'component');
  const protocolBlocks = document.blocks.filter((block) => block.name === 'protocol');
  invariant(componentBlocks.length > 0, 'compatibility lock requires components');
  invariant(protocolBlocks.length > 0, 'compatibility lock requires protocols');
  invariant(
    document.blocks
      .slice(1, 1 + componentBlocks.length)
      .every((block) => block.name === 'component') &&
      document.blocks.slice(1 + componentBlocks.length).every((block) => block.name === 'protocol'),
    'blocks must be ordered as stack, components, then protocols',
  );
  validateSorted(
    componentBlocks.map((block) => block.labels[0]),
    'component blocks',
  );
  validateSorted(
    protocolBlocks.map((block) => block.labels[0]),
    'protocol blocks',
  );

  const componentIds = new Set();
  const componentPaths = new Set();
  const components = componentBlocks.map((block) => {
    validateAttributes(
      block,
      ['owner', 'path', 'source', 'version'],
      ['manifest', 'package', 'repository', 'revision'],
    );
    const id = block.labels[0];
    const path = attribute(block, 'path', 'String');
    const sourceKind = attribute(block, 'source', 'String');
    const component = {
      id,
      manifest: optionalAttribute(block, 'manifest', 'String') ?? 'Cargo.toml',
      owner: attribute(block, 'owner', 'String'),
      package: optionalAttribute(block, 'package', 'String'),
      path,
      repository: optionalAttribute(block, 'repository', 'String'),
      revision: optionalAttribute(block, 'revision', 'String'),
      source: sourceKind,
      version: attribute(block, 'version', 'String'),
    };
    invariant(/^[a-z][a-z0-9-]*$/.test(id), `invalid component label ${id}`);
    invariant(!componentIds.has(id), `duplicate component ${id}`);
    componentIds.add(id);
    validateRelativePath(path, `component "${id}" path`);
    validateRelativePath(component.manifest, `component "${id}" manifest`);
    invariant(
      component.manifest === 'Cargo.toml' || component.manifest.endsWith('/Cargo.toml'),
      `component "${id}" manifest must name Cargo.toml`,
    );
    invariant(!componentPaths.has(path), `duplicate component path ${path}`);
    componentPaths.add(path);
    invariant(
      /^A3S-Lab\/[A-Za-z0-9._-]+$/.test(component.owner),
      `component "${id}" has invalid owner`,
    );
    invariant(
      /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(component.version),
      `component "${id}" has invalid version`,
    );
    invariant(
      sourceKind === 'git' || sourceKind === 'workspace',
      `component "${id}" source must be git or workspace`,
    );
    if (sourceKind === 'git') {
      invariant(
        /^git@github\.com:A3S-Lab\/[A-Za-z0-9._-]+\.git$/.test(component.repository ?? ''),
        `component "${id}" requires an A3S-Lab SSH repository`,
      );
      invariant(
        /^[0-9a-f]{40}$/.test(component.revision ?? ''),
        `component "${id}" requires a full lowercase revision`,
      );
    } else {
      invariant(
        component.repository === undefined && component.revision === undefined,
        `workspace component "${id}" cannot declare repository or revision`,
      );
    }
    return component;
  });

  const protocolIds = new Set();
  const protocolSchemas = new Set();
  const protocols = protocolBlocks.map((block) => {
    validateAttributes(block, ['level', 'owner', 'schema', 'source']);
    const protocol = {
      id: block.labels[0],
      level: attribute(block, 'level', 'Number'),
      owner: attribute(block, 'owner', 'String'),
      schema: attribute(block, 'schema', 'String'),
      source: attribute(block, 'source', 'String'),
    };
    invariant(/^[a-z][a-z0-9-]*$/.test(protocol.id), `invalid protocol label ${protocol.id}`);
    invariant(!protocolIds.has(protocol.id), `duplicate protocol ${protocol.id}`);
    protocolIds.add(protocol.id);
    invariant(
      Number.isSafeInteger(protocol.level) && protocol.level > 0,
      `protocol "${protocol.id}" level must be a positive integer`,
    );
    const version = /(?:^|[./])v(\d+)(?:(?:alpha|beta|rc)\d+)?$/.exec(protocol.schema);
    invariant(
      version && Number(version[1]) === protocol.level,
      `protocol "${protocol.id}" schema does not match level ${protocol.level}`,
    );
    invariant(!protocolSchemas.has(protocol.schema), `duplicate protocol schema ${protocol.schema}`);
    protocolSchemas.add(protocol.schema);
    validateRelativePath(protocol.source, `protocol "${protocol.id}" source`);
    return protocol;
  });

  const canonical = `${generate(document)}\n`;
  invariant(source === canonical, 'compatibility lock is not in canonical a3s-acl form');
  const digest = `sha256:${createHash('sha256').update(canonical).digest('hex')}`;
  return {
    stack: {
      format: attribute(stack, 'format', 'Number'),
      owner: attribute(stack, 'owner', 'String'),
      rustVersion: attribute(stack, 'rust_version', 'String'),
      schema: attribute(stack, 'schema', 'String'),
    },
    components,
    protocols,
    canonical,
    digest,
  };
}

function run(command, args, cwd, options = {}) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.status !== 0 && !options.allowFailure) {
    const detail = (result.stderr || result.stdout || '').trim();
    throw new Error(`${command} ${args.join(' ')} failed${detail ? `: ${detail}` : ''}`);
  }
  return result;
}

function git(root, args, options) {
  return run('git', args, root, options);
}

function readGitBlob(root, path, label) {
  const result = spawnSync('git', ['show', `HEAD:${path}`], {
    cwd: root,
    encoding: null,
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.status !== 0) {
    const detail = (result.stderr ?? Buffer.alloc(0)).toString('utf8').trim();
    throw new Error(`could not read committed ${label}${detail ? `: ${detail}` : ''}`);
  }
  return result.stdout;
}

function readManifestIdentity(manifestPath, componentRoot) {
  const source = readFileSync(manifestPath, 'utf8');
  if (!/^\[package\]\s*$/m.test(source)) {
    const section = tomlSection(source, 'workspace.package', manifestPath);
    const version = quotedTomlValue(section, 'version', manifestPath);
    return { name: undefined, source, version };
  }

  const section = tomlSection(source, 'package', manifestPath);
  const name = quotedTomlValue(section, 'name', manifestPath);
  const explicitVersion = optionalQuotedTomlValue(section, 'version');
  let version = explicitVersion;
  if (!version) {
    invariant(
      /^version\.workspace\s*=\s*true\s*$/m.test(section),
      `${manifestPath} is missing a quoted version or version.workspace = true`,
    );
    version = inheritedWorkspaceVersion(manifestPath, componentRoot);
  }
  return { name, source, version };
}

function inheritedWorkspaceVersion(manifestPath, componentRoot) {
  let directory = dirname(manifestPath);
  while (directory === componentRoot || directory.startsWith(`${componentRoot}${sep}`)) {
    const candidate = join(directory, 'Cargo.toml');
    if (existsSync(candidate)) {
      const source = readFileSync(candidate, 'utf8');
      if (/^\[workspace\.package\]\s*$/m.test(source)) {
        const section = tomlSection(source, 'workspace.package', candidate);
        const version = optionalQuotedTomlValue(section, 'version');
        if (version) {
          return version;
        }
      }
    }
    if (directory === componentRoot) {
      break;
    }
    directory = dirname(directory);
  }
  throw new Error(`${manifestPath} inherits a workspace version that could not be resolved`);
}

function tomlSection(source, name, label) {
  const escapedName = name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = new RegExp(
    `^\\[${escapedName}\\]\\s*$([\\s\\S]*?)(?=^\\[|(?![\\s\\S]))`,
    'm',
  ).exec(source);
  invariant(match, `${label} is missing [${name}]`);
  return match[1];
}

function quotedTomlValue(section, name, label) {
  const value = optionalQuotedTomlValue(section, name);
  invariant(value, `${label} is missing a quoted ${name}`);
  return value;
}

function optionalQuotedTomlValue(section, name) {
  const match = new RegExp(
    `^${name.replaceAll('-', '\\-')}\\s*=\\s*"([^"]+)"\\s*$`,
    'm',
  ).exec(section);
  return match?.[1];
}

export function tomlDependency(source, sectionName, dependency, label = 'Cargo.toml') {
  const section = tomlSection(source, sectionName, label);
  const lines = section.split('\n');
  const start = lines.findIndex((line) =>
    new RegExp(`^${dependency.replaceAll('-', '\\-')}\\s*=`).test(line),
  );
  invariant(start >= 0, `${label} [${sectionName}] is missing ${dependency}`);
  let declaration = lines[start].slice(lines[start].indexOf('=') + 1).trim();
  let depth = bracketDepth(declaration);
  let index = start + 1;
  while (depth > 0 && index < lines.length) {
    declaration += ` ${lines[index].trim()}`;
    depth = bracketDepth(declaration);
    index += 1;
  }
  invariant(depth === 0, `${label} has an incomplete ${dependency} declaration`);
  return declaration.replace(/\s+/g, ' ').trim();
}

function bracketDepth(value) {
  let depth = 0;
  let quoted = false;
  let escaped = false;
  for (const character of value) {
    if (escaped) {
      escaped = false;
    } else if (character === '\\' && quoted) {
      escaped = true;
    } else if (character === '"') {
      quoted = !quoted;
    } else if (!quoted && (character === '{' || character === '[')) {
      depth += 1;
    } else if (!quoted && (character === '}' || character === ']')) {
      depth -= 1;
    }
  }
  return depth;
}

function optionalDependencyField(declaration, field) {
  const match = new RegExp(`\\b${field}\\s*=\\s*"([^"]+)"`).exec(declaration);
  return match?.[1];
}

function dependencyField(declaration, field, label) {
  const value = optionalDependencyField(declaration, field);
  invariant(value, `${label} is missing ${field}`);
  return value;
}

function exactDependencyVersion(declaration, label) {
  const direct = /^"([^"]+)"$/.exec(declaration);
  const version = direct?.[1] ?? dependencyField(declaration, 'version', label);
  invariant(version.startsWith('='), `${label} must use an exact =version requirement`);
  return version.slice(1);
}

export function packageFromLock(
  source,
  name,
  expectedVersion,
  label,
  expectedRevision,
  expectedSource,
) {
  const entries = source.split('[[package]]').slice(1);
  const entry = entries.find((candidate) => {
    if (
      !new RegExp(`^\\s*name = "${name}"\\s*$`, 'm').test(candidate) ||
      quotedTomlValue(candidate, 'version', `${label} package ${name}`) !== expectedVersion
    ) {
      return false;
    }
    const packageSource = /^source = "([^"]+)"$/m.exec(candidate)?.[1];
    if (expectedRevision && !packageSource?.endsWith(`#${expectedRevision}`)) {
      return false;
    }
    return !expectedSource || packageSource === expectedSource;
  });
  const revisionLabel = expectedRevision
    ? ` at revision ${expectedRevision}`
    : '';
  const sourceLabel = expectedSource ? ` from ${expectedSource}` : '';
  invariant(
    entry,
    `${label} is missing package ${name} ${expectedVersion}${revisionLabel}${sourceLabel}`,
  );
  return {
    version: quotedTomlValue(entry, 'version', `${label} package ${name}`),
    source: /^source = "([^"]+)"$/m.exec(entry)?.[1],
    checksum: /^checksum = "([^"]+)"$/m.exec(entry)?.[1],
  };
}

function assertLockVersion(lockSource, component, label, expectedRevision) {
  const entry = packageFromLock(
    lockSource,
    component.package,
    component.version,
    label,
    expectedRevision,
  );
  invariant(
    entry.version === component.version,
    `${label} locks ${component.package} ${entry.version}, expected ${component.version}`,
  );
  if (expectedRevision) {
    invariant(
      entry.source?.endsWith(`#${expectedRevision}`),
      `${label} does not lock ${component.package} revision ${expectedRevision}`,
    );
  }
}

function assertPublishedLockVersion(lockSource, component, label) {
  const entry = packageFromLock(
    lockSource,
    component.package,
    component.version,
    label,
    undefined,
    CRATES_IO_SOURCE,
  );
  invariant(
    /^[0-9a-f]{64}$/.test(entry.checksum ?? ''),
    `${label} package ${component.package} is missing a crates.io checksum`,
  );
}

function verifyPublishedDependency(declaration, component, cloudLock) {
  const label = `Cloud ${component.package}`;
  invariant(
    exactDependencyVersion(declaration, label) === component.version,
    `${label} version does not match the compatibility lock`,
  );
  invariant(
    ['git', 'rev', 'path', 'registry'].every(
      (field) => optionalDependencyField(declaration, field) === undefined,
    ),
    `${label} must resolve from the published registry release`,
  );
  assertPublishedLockVersion(cloudLock, component, 'apps/cloud/Cargo.lock');
}

function verifyDependencyBindings(root, componentMap) {
  const cloud = componentMap.get('cloud');
  const gateway = componentMap.get('gateway');
  const use = componentMap.get('use');
  const cloudManifestPath = join(root, cloud.path, 'Cargo.toml');
  const cloudManifest = readFileSync(cloudManifestPath, 'utf8');
  const cloudLock = readFileSync(join(root, cloud.path, 'Cargo.lock'), 'utf8');

  for (const id of ['acl', 'boot', 'event']) {
    const component = componentMap.get(id);
    const declaration = tomlDependency(
      cloudManifest,
      'workspace.dependencies',
      component.package,
      cloudManifestPath,
    );
    invariant(
      exactDependencyVersion(declaration, `Cloud ${component.package}`) === component.version,
      `Cloud ${component.package} does not match the compatibility lock`,
    );
    assertLockVersion(cloudLock, component, 'apps/cloud/Cargo.lock');
  }

  for (const id of ['box', 'form', 'orm']) {
    const component = componentMap.get(id);
    const declaration = tomlDependency(
      cloudManifest,
      'workspace.dependencies',
      component.package,
      cloudManifestPath,
    );
    invariant(
      dependencyField(declaration, 'git', `Cloud ${component.package}`) ===
        component.repository.replace('git@github.com:', 'https://github.com/'),
      `Cloud ${component.package} repository does not match the compatibility lock`,
    );
    invariant(
      dependencyField(declaration, 'rev', `Cloud ${component.package}`) === component.revision,
      `Cloud ${component.package} revision does not match the compatibility lock`,
    );
    if (id !== 'orm') {
      invariant(
        exactDependencyVersion(declaration, `Cloud ${component.package}`) === component.version,
        `Cloud ${component.package} version does not match the compatibility lock`,
      );
    }
    assertLockVersion(cloudLock, component, 'apps/cloud/Cargo.lock', component.revision);
  }

  for (const id of ['code', 'flow']) {
    const component = componentMap.get(id);
    const declaration = tomlDependency(
      cloudManifest,
      'workspace.dependencies',
      component.package,
      cloudManifestPath,
    );
    verifyPublishedDependency(declaration, component, cloudLock);
  }

  const runtime = componentMap.get('runtime');
  const runtimeDeclaration = tomlDependency(
    cloudManifest,
    'workspace.dependencies',
    runtime.package,
    cloudManifestPath,
  );
  invariant(
    exactDependencyVersion(runtimeDeclaration, 'Cloud a3s-runtime') === runtime.version,
    'Cloud a3s-runtime version does not match the compatibility lock',
  );
  invariant(
    dependencyField(runtimeDeclaration, 'git', 'Cloud a3s-runtime') ===
      runtime.repository.replace('git@github.com:', 'https://github.com/'),
    'Cloud a3s-runtime repository does not match the compatibility lock',
  );
  invariant(
    dependencyField(runtimeDeclaration, 'rev', 'Cloud a3s-runtime') === runtime.revision,
    'Cloud a3s-runtime revision does not match the compatibility lock',
  );
  assertLockVersion(cloudLock, runtime, 'apps/cloud/Cargo.lock', runtime.revision);

  const useRoot = join(root, use.path);
  for (const manifest of ['crates/core/Cargo.toml', 'crates/extension/Cargo.toml']) {
    const manifestPath = join(useRoot, manifest);
    const identity = readManifestIdentity(manifestPath, useRoot);
    const declaration = tomlDependency(
      cloudManifest,
      'workspace.dependencies',
      identity.name,
      cloudManifestPath,
    );
    invariant(
      exactDependencyVersion(declaration, `Cloud ${identity.name}`) === identity.version,
      `Cloud ${identity.name} does not match the locked Use package version`,
    );
    invariant(
      dependencyField(declaration, 'git', `Cloud ${identity.name}`) ===
        use.repository.replace('git@github.com:', 'https://github.com/'),
      `Cloud ${identity.name} repository does not match the compatibility lock`,
    );
    invariant(
      dependencyField(declaration, 'rev', `Cloud ${identity.name}`) === use.revision,
      `Cloud ${identity.name} revision does not match the compatibility lock`,
    );
    assertLockVersion(
      cloudLock,
      { package: identity.name, version: identity.version },
      'apps/cloud/Cargo.lock',
      use.revision,
    );
  }

  const gatewayManifestPath = join(root, gateway.path, 'Cargo.toml');
  const gatewayManifest = readFileSync(gatewayManifestPath, 'utf8');
  for (const id of ['acl', 'updater', 'sentry']) {
    const component = componentMap.get(id);
    const declaration = tomlDependency(
      gatewayManifest,
      'dependencies',
      component.package,
      gatewayManifestPath,
    );
    invariant(
      exactDependencyVersion(declaration, `Gateway ${component.package}`) === component.version,
      `Gateway ${component.package} does not match the compatibility lock`,
    );
  }
  const gatewayLock = readFileSync(join(root, gateway.path, 'Cargo.lock'), 'utf8');
  for (const id of ['acl', 'updater', 'sentry']) {
    assertLockVersion(gatewayLock, componentMap.get(id), 'crates/gateway/Cargo.lock');
  }
}

function verifyAclConfiguration(root, cloudPath) {
  const listed = git(join(root, cloudPath), [
    'ls-files',
    '-z',
    '--',
    'README.md',
    'config',
    'crates',
    'deploy',
    'docs',
    'web',
  ]).stdout;
  const files = listed.split('\0').filter(Boolean);
  const forbidden = files.filter((file) => /\.(?:hcl|tf|tfvars)$/i.test(file));
  invariant(
    forbidden.length === 0,
    `Cloud product configuration must be ACL; forbidden files: ${forbidden.join(', ')}`,
  );

  for (const file of files) {
    const bytes = readFileSync(join(root, cloudPath, file));
    if (bytes.includes(0)) {
      continue;
    }
    const source = bytes.toString('utf8');
    invariant(
      !/\bHCL\b|\.hcl\b/i.test(source),
      `${cloudPath}/${file} contains an HCL product-configuration reference`,
    );
  }

  const aclFiles = files.filter((file) => file.startsWith('config/') && file.endsWith('.acl'));
  invariant(aclFiles.length > 0, 'Cloud must include tracked ACL configuration fixtures');
  for (const file of aclFiles) {
    const source = readFileSync(join(root, cloudPath, file), 'utf8');
    const document = parse(source);
    const generated = generate(document);
    invariant(
      generate(parse(generated)) === generated,
      `${cloudPath}/${file} does not reach a stable a3s-acl parse/generate form`,
    );
  }
  return aclFiles;
}

function verifyFormFixture(
  root,
  cloudPath,
  form,
  ownerFileName,
  cloudFileName,
  label,
) {
  const nestedCratesIndex = form.manifest.lastIndexOf('/crates/');
  let formModuleRoot = null;
  if (nestedCratesIndex >= 0) {
    formModuleRoot = form.manifest.slice(0, nestedCratesIndex);
  } else if (form.manifest.startsWith('crates/')) {
    formModuleRoot = '';
  }
  invariant(
    formModuleRoot !== null,
    `Form manifest must be rooted in a crates directory: ${form.manifest}`,
  );
  const ownerRepositoryRelativePath = [
    formModuleRoot,
    'tests/conformance',
    ownerFileName,
  ]
    .filter(Boolean)
    .join('/');
  const ownerRelativePath = `${form.path}/${ownerRepositoryRelativePath}`;
  const cloudRelativePath = `${cloudPath}/crates/control-plane/tests/fixtures/${cloudFileName}`;
  const ownerPath = join(root, ownerRelativePath);
  const cloudFixturePath = join(root, cloudRelativePath);
  invariant(existsSync(ownerPath), `${label} fixture is missing: ${ownerRelativePath}`);
  invariant(
    existsSync(cloudFixturePath),
    `Cloud ${label.toLowerCase()} fixture is missing: ${cloudRelativePath}`,
  );
  const ownerBytes = readGitBlob(
    join(root, form.path),
    ownerRepositoryRelativePath,
    `${label} fixture`,
  );
  const cloudBytes = readGitBlob(
    join(root, cloudPath),
    `crates/control-plane/tests/fixtures/${cloudFileName}`,
    `Cloud ${label.toLowerCase()} fixture`,
  );
  invariant(
    ownerBytes.equals(cloudBytes),
    `Cloud ${label.toLowerCase()} fixture must be byte-identical to ${ownerRelativePath}`,
  );
  return {
    cloud: cloudRelativePath,
    digest: `sha256:${createHash('sha256').update(ownerBytes).digest('hex')}`,
    owner: ownerRelativePath,
  };
}

export function verifyCloudStack(root = DEFAULT_ROOT, lockRelativePath = 'compat/cloud-stack.acl') {
  const lockPath = join(root, lockRelativePath);
  invariant(existsSync(lockPath), `missing compatibility lock ${lockRelativePath}`);
  const lock = parseCloudStackLock(readFileSync(lockPath, 'utf8'));
  const componentMap = new Map(lock.components.map((component) => [component.id, component]));
  for (const id of REQUIRED_COMPONENTS) {
    invariant(componentMap.has(id), `compatibility lock is missing required component ${id}`);
  }

  const rootRevision = git(root, ['rev-parse', 'HEAD']).stdout.trim();
  const resolvedComponents = [];
  for (const component of lock.components) {
    const componentRoot = join(root, component.path);
    invariant(existsSync(componentRoot), `component "${component.id}" is not initialized`);
    const manifestPath = join(componentRoot, component.manifest);
    invariant(existsSync(manifestPath), `component "${component.id}" is missing Cargo.toml`);
    const identity = readManifestIdentity(manifestPath, componentRoot);
    invariant(
      identity.version === component.version,
      `component "${component.id}" manifest is ${identity.version}, expected ${component.version}`,
    );
    if (component.package) {
      invariant(
        identity.name === component.package,
        `component "${component.id}" package is ${identity.name}, expected ${component.package}`,
      );
    }

    let resolvedRevision = rootRevision;
    if (component.source === 'git') {
      const modulePath = git(root, [
        'config',
        '-f',
        '.gitmodules',
        '--get',
        `submodule.${component.path}.path`,
      ]).stdout.trim();
      const moduleUrl = git(root, [
        'config',
        '-f',
        '.gitmodules',
        '--get',
        `submodule.${component.path}.url`,
      ]).stdout.trim();
      invariant(modulePath === component.path, `submodule ${component.path} is not registered`);
      invariant(
        moduleUrl === component.repository,
        `submodule ${component.path} URL is ${moduleUrl}, expected ${component.repository}`,
      );
      const indexEntry = git(root, ['ls-files', '--stage', '--', component.path]).stdout.trim();
      const match = /^160000 ([0-9a-f]{40}) 0\t/.exec(indexEntry);
      invariant(match, `component "${component.id}" is not recorded as a gitlink`);
      invariant(
        match[1] === component.revision,
        `component "${component.id}" gitlink is ${match[1]}, expected ${component.revision}`,
      );
      resolvedRevision = git(componentRoot, ['rev-parse', 'HEAD']).stdout.trim();
      invariant(
        resolvedRevision === component.revision,
        `component "${component.id}" HEAD is ${resolvedRevision}, expected ${component.revision}`,
      );
      const dirty = git(componentRoot, ['status', '--porcelain=v1', '--untracked-files=all']).stdout;
      invariant(dirty.length === 0, `component "${component.id}" worktree is dirty`);
    } else {
      const tracked =
        git(
          root,
          [
            'ls-files',
            '--error-unmatch',
            '--',
            `${component.path}/${component.manifest}`,
          ],
          { allowFailure: true },
        ).status === 0;
      invariant(tracked, `workspace component "${component.id}" is not tracked`);
      const dirty = git(root, [
        'status',
        '--porcelain=v1',
        '--untracked-files=all',
        '--',
        component.path,
      ]).stdout;
      invariant(dirty.length === 0, `workspace component "${component.id}" is dirty`);
    }
    resolvedComponents.push({ ...component, resolvedRevision });
  }

  for (const protocol of lock.protocols) {
    const sourcePath = join(root, protocol.source);
    invariant(existsSync(sourcePath), `protocol "${protocol.id}" source is missing`);
    const source = readFileSync(sourcePath, 'utf8');
    invariant(
      source.includes(`"${protocol.schema}"`),
      `protocol "${protocol.id}" source does not declare ${protocol.schema}`,
    );
  }

  verifyDependencyBindings(root, componentMap);
  const formInteractionFixture = verifyFormFixture(
    root,
    componentMap.get('cloud').path,
    componentMap.get('form'),
    'interaction-contract-v1.json',
    'form-interaction-contract-v1.json',
    'Form interaction',
  );
  const formValueEvaluationFixture = verifyFormFixture(
    root,
    componentMap.get('cloud').path,
    componentMap.get('form'),
    'value-evaluation-v1.json',
    'form-value-evaluation-v1.json',
    'Form value evaluation',
  );
  const aclFiles = verifyAclConfiguration(root, componentMap.get('cloud').path);
  return {
    ...lock,
    components: resolvedComponents,
    aclFiles,
    formInteractionFixture,
    formValueEvaluationFixture,
    rootRevision,
  };
}

export function formatVerification(result) {
  const lines = [
    `Cloud compatibility lock ${result.digest}`,
    `Rust ${result.stack.rustVersion}`,
  ];
  for (const component of result.components) {
    lines.push(
      `${component.id}: ${component.resolvedRevision} (${component.version}, ${component.path})`,
    );
  }
  for (const protocol of result.protocols) {
    lines.push(`${protocol.id}: ${protocol.schema}`);
  }
  lines.push(
    `Form interaction fixture: ${result.formInteractionFixture.digest} ` +
      `(${result.formInteractionFixture.owner} == ${result.formInteractionFixture.cloud})`,
  );
  lines.push(
    `Form value evaluation fixture: ${result.formValueEvaluationFixture.digest} ` +
      `(${result.formValueEvaluationFixture.owner} == ` +
      `${result.formValueEvaluationFixture.cloud})`,
  );
  lines.push(`ACL fixtures: ${result.aclFiles.join(', ')}`);
  return lines.join('\n');
}

function main() {
  const rootArgument = process.argv.indexOf('--root');
  const root =
    rootArgument >= 0
      ? resolve(process.argv[rootArgument + 1] ?? '')
      : DEFAULT_ROOT;
  const result = verifyCloudStack(root);
  process.stdout.write(`${formatVerification(result)}\n`);
}

if (resolve(process.argv[1] ?? '') === SCRIPT_PATH) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`cloud-stack verification failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
