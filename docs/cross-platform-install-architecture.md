# A3S Cross-Platform Install Technical Architecture

- Status: Proposed, pre-1.0
- Date: 2026-07-15
- Parent: [A3S Component Management Design](component-management-design.md)
- Related: [Product Design](cross-platform-install-product.md) and
  [A3S Use Extension Design](a3s-use-extension-design.md)

## 1. System Architecture

```text
user / CI / agent
        |
        v
umbrella CLI: parse, policy, UX, approval
        |
        v
component application layer
  catalog -> target probe -> resolver -> planner -> executor
      |                         |             |
      v                         v             v
signed metadata           versioned plan   transaction journal
                                                |
                  +-----------------------------+-------------------+
                  |                 |                 |             |
                  v                 v                 v             v
          managed artifact    native manager     delegated CLI  local package
          tar/zip/binary      brew/winget/...    a3s-use        ACL extension
                  |                 |                 |             |
                  +-----------------+-----------------+-------------+
                                                |
                                                v
                                      verify -> receipt -> status
```

Ownership remains split:

| Layer | Owns |
| --- | --- |
| `a3s` CLI | Public commands, policy, approval, plan presentation, exit behavior |
| Component application layer | Catalog merge, target resolution, source selection, dependency DAG, orchestration |
| `a3s-updater` | Download, digest verification, archive safety, activation, rollback, receipt primitives |
| Built-in backend | Typed translation from a source manifest to exact process/API operations |
| Native manager | Dependency solving, system package database, package files, native uninstall |
| Trusted parent such as Use | Child component semantics and provider-specific lifecycle |

The component application layer remains in the CLI repository until a second
consumer requires a stable library. `a3s-updater` remains a mechanics library;
it does not own product catalogs, source policy, or interactive UX.

## 2. Core Types

The implementation should replace backend-specific branches with typed
objects:

```rust
struct InstallRequest {
    component: ComponentId,
    version: Option<VersionRequirement>,
    channel: ReleaseChannel,
    source: Option<SourceId>,
    scope: Option<InstallScope>,
    offline: bool,
    force: bool,
    migrate: bool,
}

struct SourceCandidate {
    id: SourceId,
    kind: SourceKind,
    target: TargetConstraint,
    package: PackageReference,
    integrity: IntegrityRequirement,
    priority: u16,
}

struct InstallPlan {
    schema_version: u32,
    plan_digest: String,
    component: ResolvedComponent,
    target: Target,
    source: ResolvedSource,
    scope: InstallScope,
    actions: Vec<PlannedAction>,
    permissions: Vec<RequiredPermission>,
    risks: Vec<InstallRisk>,
    ownership: OwnershipModel,
    rollback: RollbackCapability,
}
```

Public types are bounded, validated, and `Send + Sync` where applicable.
Package references are backend-specific typed enums; they are not arbitrary
command arguments. The plan digest covers a canonical, versioned representation
with volatile timestamps and secrets excluded.

## 3. Backend Contract

```rust
#[async_trait::async_trait]
trait InstallerBackend: Send + Sync {
    fn id(&self) -> BackendId;
    fn capabilities(&self) -> BackendCapabilities;

    async fn probe(&self, target: &Target) -> Result<BackendStatus>;
    async fn resolve(
        &self,
        candidate: &SourceCandidate,
        request: &InstallRequest,
        context: &ResolveContext,
    ) -> Result<ResolvedSource>;
    async fn plan(
        &self,
        source: &ResolvedSource,
        current: Option<&ComponentReceipt>,
    ) -> Result<BackendPlan>;
    async fn apply(&self, plan: &BackendPlan, journal: &mut Journal)
        -> Result<BackendResult>;
    async fn verify(&self, result: &BackendResult, probe: &HealthProbe)
        -> Result<VerifiedInstall>;
    async fn uninstall(
        &self,
        receipt: &ComponentReceipt,
        journal: &mut Journal,
    ) -> Result<BackendResult>;
}
```

Only a backend may construct its process arguments. Execution uses an absolute
executable path and argv through async process I/O, never a shell. The backend
validates package IDs, repositories, sources, scopes, and returned state before
recording success. Backend-wide operations also honor any native global lock;
A3S never bypasses or deletes another manager's lock.

## 4. Backend Families

### 4.1 Managed Artifact

This backend supports bounded tar, zip, and single-binary artifacts. It:

1. downloads into an A3S cache;
2. verifies publisher digest and signature metadata;
3. extracts into an owned staging root with traversal and size limits;
4. never runs package-provided install scripts;
5. validates executable identity and version;
6. atomically activates a version root;
7. records exactly which root A3S owns.

It is the portable fallback and the only Tier 1 backend A3S can make fully
rollback-capable across all three operating systems.

### 4.2 Native Package Manager

Homebrew, Winget, APT, DNF, Pacman, and Zypper adapters use manager-native
query, install, update, and uninstall operations. They record package and
repository identity but never enumerate manager-owned files as A3S-owned.

Before execution the plan reports:

- the exact manager and package identity;
- whether repository refresh or dependency changes may occur;
- user versus system scope;
- expected privilege behavior;
- whether package scripts or native installers may execute;
- the manager's rollback and version-selection limits.

After execution the adapter queries the manager again and performs the
component health probe. Process exit success alone is insufficient.

### 4.3 Native Installer Format

MSI, MSIX, PKG, DEB, and RPM are supported only through typed built-in
adapters. When a native package manager can apply a local package while
preserving dependency and ownership records, that path is preferred. A3S does
not treat native package contents as directly owned files.

### 4.4 Delegated Component

`use/browser`, `use/office`, and external Use domains remain owned by the
trusted `a3s-use` parent. The root passes normalized request constraints and
receives versioned plan and result JSON over an ordinary CLI process contract.
It does not learn Chrome, OfficeCLI, CLI, MCP, or Skill internals.

### 4.5 Local ACL Package

`--from` accepts an explicit local package supported by the trusted parent or
local-package backend. Unsigned packages require `--allow-unsigned`, remain
`local-explicit`, and never gain automatic network update behavior.

## 5. Transactions and Recovery

Every mutation has a per-component lock and a durable journal:

```text
planned -> acquiring -> staged -> applied -> verified -> committed
                          |          |           |
                          +----------+-----------+-> recovering | outcome-unknown
```

For A3S-owned artifacts, failure before commit removes staging and preserves
the previous active version. Activation and receipt replacement are ordered so
the old receipt remains authoritative until the new version verifies.

For native package managers, process interruption may leave an ambiguous
outcome. A3S must re-query manager state and health before deciding. If state
cannot be proven, it records `outcome-unknown`, preserves the recovery journal,
and does not automatically retry a mutating operation.

Dependency operations form a DAG, but each component receipt commits
independently. A failed child does not cause A3S to pretend it can roll back a
successful system package operation.

## 6. Receipts and Ownership

Receipts remain versioned machine-owned JSON. A target receipt records at
least:

- component ID, resolved version, channel, and target;
- catalog and registry snapshot identity;
- source ID, backend ID, package reference, and scope;
- publisher identity, artifact digest, signature evidence, and source URL when
  applicable;
- ownership model and A3S-owned roots, if any;
- native manager package and repository identity, if delegated;
- executable and health-probe identity;
- requested version and source constraints;
- dependency and parent relationships;
- plan digest, transaction ID, install time, and last verified time;
- backend capability and rollback summary.

Ownership is explicit:

| Ownership | Mutation rule |
| --- | --- |
| `a3s` | Delete only validated roots under the approved A3S data directory. |
| `package-manager` | Invoke the same recorded manager and verify post-state. |
| `parent` | Delegate to the same compatible parent CLI contract. |
| `external` | Never mutate; provide manual instructions. |
| `none` | No files exist or the component is bundled and non-removable. |

Receipts are evidence, not configuration. Human policy remains in ACL.

Shared path resolution honors `A3S_DATA_HOME`, `A3S_STATE_HOME`, and
`A3S_CACHE_HOME` first. Defaults are platform-native:

| Platform | Data | State | Cache |
| --- | --- | --- | --- |
| macOS | `~/Library/Application Support/A3S` | `~/Library/Application Support/A3S/State` | `~/Library/Caches/A3S` |
| Linux | `$XDG_DATA_HOME/a3s` | `$XDG_STATE_HOME/a3s` | `$XDG_CACHE_HOME/a3s` |
| Windows | `%LOCALAPPDATA%\A3S\Data` | `%LOCALAPPDATA%\A3S\State` | `%LOCALAPPDATA%\A3S\Cache` |

Existing roots remain readable during migration. A receipt or owned directory
is moved only after identity and ownership validation; path migration never
adopts an external installation.

## 7. Scope, Privileges, and Policy

User scope is preferred whenever a compatible backend supports it. Managed
artifacts install only into user-controlled A3S roots in the first stable
release.

System scope requires an explicit request or policy. A3S never silently starts
`sudo` or bypasses UAC. An interactive native manager may display its own
privilege prompt after the A3S plan identifies that possibility. Non-interactive
execution fails before mutation if required privilege is unavailable.

Example user policy:

```acl
install {
  default_scope = "user"
  preferred_sources = ["same-provenance", "native-manager", "managed-release"]
  allow_prerelease = false
  allow_source_migration = false

  registry "a3s" {
    url = "https://components.a3s.dev/"
    trust_root = "sha256:<root-metadata-digest>"
  }
}
```

The configuration is parsed and generated with `a3s-acl`.

## 8. Trust and Security

- Registry metadata has freshness, freeze, and rollback protection.
- Direct artifacts require publisher-provided integrity. A checksum calculated
  only after an untrusted download is not sufficient verification.
- Native manager sources pin expected package and repository identities.
- Registry manifests cannot inject commands, arguments, environment variables,
  probes, or scripts outside typed schema fields.
- Health probes use catalog-approved typed conventions, are bounded, and run
  only after source trust validation.
- Download redirects, archive sizes, entry counts, extracted sizes, and paths
  are bounded.
- Package-manager execution receives a minimal environment and uses no shell.
- A plan explicitly marks backends that may execute package scripts.
- Logs and receipts never store credentials, authorization headers, or secret
  query parameters.
- Uninstall validates the current receipt and ownership boundary again before
  mutation.

## 9. Delegated CLI Contract Evolution

The current delegated install/status/uninstall CLI schema remains compatible.
A later schema adds planning and plan-digest protection:

```text
a3s-use component plan browser --source managed-chrome --json
a3s-use component install browser --expected-plan <sha256> --json
a3s-use component status browser --json
a3s-use component uninstall browser --json
```

The child recomputes the plan and rejects execution if the expected digest no
longer matches. This prevents a source or privilege change between approval and
execution. It is versioned CLI JSON, not JSON-RPC.

## 10. Offline and Enterprise Operation

`--offline` permits only cached registry metadata, cached verified artifacts,
installed native-manager metadata, and explicit local packages. It never
silently falls back to the network.

The download layer honors explicit proxy and custom CA configuration without
passing credentials into receipts. Enterprises may mirror TUF metadata and
artifacts while retaining publisher digests and signatures. Native package
manager proxy and repository policy remains manager-owned.

## 11. Output and Errors

Plans and results use separate versioned JSON schemas. Important stable errors
include:

```text
component.unsupported_target
component.source_unavailable
component.source_migration_required
component.backend_capability_missing
component.privilege_required
component.integrity_unavailable
component.integrity_mismatch
component.plan_changed
component.install_outcome_unknown
component.not_owned
component.offline_cache_miss
component.registry_expired
```

Errors include rejected source candidates and actionable alternatives without
leaking sensitive URLs or environment values.

## 12. Delivery Sequence

### Phase 1: Normalize the Existing Engine

- introduce `Target`, `SourceId`, `SourceCandidate`, capabilities, and plan
  types;
- implement `a3s info`, `--dry-run`, and source-selection explanations;
- preserve current Homebrew and GitHub behavior behind backends;
- expand receipt schema with migration tests.

### Phase 2: macOS and Linux Portable Foundation

- support tar, zip, and single-file artifacts;
- publish macOS and Linux artifacts with publisher digests;
- run install/upgrade/recovery/uninstall conformance tests on supported targets;
- harden glibc/musl and architecture diagnostics before adding another runtime
  platform.

### Phase 3: Windows Promotion

- add Windows executable, path, process, lock, activation, and uninstall tests;
- publish Windows artifacts with publisher digests;
- pass real Browser cross-process persistent-session and cleanup tests;
- promote Windows from compile/package preview only after those gates pass.

### Phase 4: Primary Native Managers

- stabilize Homebrew as a backend;
- add Winget with user/system scope and outcome re-probing;
- add source migration with explicit confirmation;
- verify manager-owned uninstall never directly deletes files.

### Phase 5: Linux Native Managers

- add APT and DNF based on actual A3S component packages;
- add Pacman and Zypper only with release and CI fixtures;
- test privilege denial, repository mismatch, scripts, partial outcomes, and
  dependency changes.

### Phase 6: Signed Registry

- publish the official ACL component manifests;
- deploy TUF root, targets, snapshot, and timestamp metadata;
- add explicit third-party registry enrollment and trust UX;
- enable remote external component discovery without executable installer
  plugins.

### Phase 7: Demand-Driven Ecosystem Adapters

- add Snap, Flatpak, Scoop, Chocolatey, or language-manager adapters only when
  a registered component and CI environment demonstrate the need;
- keep each adapter behind the same plan, capability, trust, receipt, and
  uninstall conformance suite.

## 13. Acceptance Criteria

The current macOS/Linux foundation is stable only when:

- one official component installs, upgrades, repairs, and uninstalls through a
  managed artifact on each supported macOS and Linux target;
- Homebrew installations remain manager-owned throughout lifecycle;
- unsupported target and capability combinations fail before mutation;
- dry-run output identifies source, scope, privileges, scripts, ownership, and
  rollback behavior;
- source selection is deterministic and provenance migration is explicit;
- a killed direct install recovers the previous healthy version;
- a killed native-manager operation is re-probed and can report
  `outcome-unknown` without automatic retry;
- registry rollback, expiration, wrong publisher, digest mismatch, and archive
  traversal tests fail safely;
- unsigned local packages require explicit trust and never auto-update;
- delegated Browser, Office, CLI, MCP, and Skill boundaries remain unchanged;
- macOS and Linux CI leave no package-manager locks, child processes, staging
  paths, or test packages behind.

Windows promotion additionally requires a published managed artifact,
ownership-preserving Winget behavior when enabled, clean interruption and file
lock recovery, and the same Browser persistent-session lifecycle evidence as
macOS and Linux.

## 14. Deferred Decisions

- multiple active versions of one product component;
- a committed project-level component lockfile;
- system-scoped A3S-owned portable installations;
- remote custom installer backend plugins;
- peer-to-peer artifact distribution;
- transactional rollback across multiple native package managers.
