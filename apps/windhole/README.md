# A3S智能体评测

A3S智能体评测 is a local, game-inspired control surface for A3S Bench. It
turns Bench Tasks into selectable maps, a hangar roster into deployable
aircraft-and-pilot combinations, real Bench jobs into per-aircraft mission
state, and exact Run IDs into battle reports. Three.js provides the tactical
presentation through a dominant outdoor battlefield, but execution progress
and scores always come from A3S Bench.

The browser never starts local processes. A loopback-only bridge accepts only
exact local browser origins, requires JSON media types for POST requests,
verifies the configured CLI reports the Bench v1 component protocol, and passes
validated argument arrays to `a3s-bench` with `shell: false`. The application
permits deployment only after the bridge health check succeeds and Doctor
reports `runtime.ready=true`. When that boundary is unavailable, the fallback
Task catalog and 3D scene remain browseable, but there is no simulated run,
generated progress, fabricated Run ID, or preview score.

## Real data loop

```text
versioned hangar roster
  aircraft + pilot + Candidate + model + effort + callsign
                    │
                    ├── active member + selected map ── single sortie
                    │
                    └── complete roster + selected map ── Campaign
                                      │
                             immutable deployment snapshot
                                      │
                         local Bridge tracking ID per sortie
                                      │
                        exact Bench Run ID from each process
                                      │
                     GET /results/:runId for the public result
                                      │
                   3D status + versioned manifest + battle report
```

The loopback bridge generates each `windhole-*` value as an in-memory tracking
ID for one local child process. It is not a Bench-issued Job ID. A Bench Run ID
becomes authoritative only when the CLI process returns it. If Bench reports an
anchored `run local-* failed: ...` terminal error, the bridge preserves that
exact failed Run ID for recovery; arbitrary error text cannot introduce one.

`HangarRosterEntry` is the source of truth for an ordinary deployment's
airframe, pilot identity, Candidate Adapter, model, visual effort/loadout, and
callsign. The map workspace edits the active entry rather than maintaining a
second Candidate/model configuration. Explicit single-sortie lock mode is the
only exception: Task Lock and Candidate Lock paths replace the ordinary Task,
Candidate, and model inputs for that run.

The selected Bench Task is the selected map. Starting a deployment freezes the
map and the applicable roster data before any request is sent, so later UI
state cannot change the attribution of an in-flight result.

## Product surface

| Workspace | User outcome | Bench capability |
| --- | --- | --- |
| Map theater | Browse ready and blocked Tasks as maps | `list --all --json` |
| Map theater | Inspect availability, admission, provenance, and an optional description | `info <task> --all --json` |
| Map theater | Deploy the active hangar member | one real `run <task> --agent <candidate> [--model …] --json` |
| Map theater | Deploy the complete hangar roster | one real `run` per member, locally queued with at most two active jobs |
| Hangar squadron | Compose and persist 1–5 deployable aircraft/pilot combinations | supplies ordinary run inputs and 3D identity |
| Battle reports | Read one exact Run ID or explicitly request the latest local result | `result <run-id> --json` or `result --json` |
| Engineering bay | Check Runtime and Judge model readiness | `advanced doctor --json` |
| Engineering bay | Validate a local TaskBundle | `advanced check <./task>` |
| Engineering bay | Create an immutable Task lock | `advanced task lock <source> --out <file>` |
| Engineering bay | Create an immutable Candidate lock | `advanced candidate lock <candidate> [--model …] --out <file>` |

## Single sorties and Campaigns

The deployment scope is explicit:

- **Single sortie** uses the active roster member. It supports ordinary
  Candidate/model input or the Task Lock + Candidate Lock workflow.
- **Campaign** freezes the selected map and the complete ordered roster. Each
  aircraft receives its own input, Bridge tracking ID, Bench Run ID, result,
  status, and error.
  Two workers submit real Bench jobs concurrently; remaining members stay
  `queued` until a worker is available.
- A member failure does not cancel the other members. The Campaign ends as
  completed, completed with failures, or failed according to the actual member
  outcomes.
- Every completed member is resolved through
  `GET /api/v1/bench/results/:runId`, and the returned `run_id` must match. The
  latest-result endpoint is never used to guess Campaign ownership.
- Lock mode is intentionally single-sortie only. Reusing one Candidate Lock as
  if it represented several roster members would destroy attribution, so the
  Campaign preflight rejects it.

While either scope is active, map selection, hangar mutation, active-member
switching, Candidate/model edits, effort changes, scope changes, and lock
changes are blocked. A Campaign scene click may still change the locally
inspected aircraft without changing the frozen active roster entry.

## Candidate and visual identity contracts

Ordinary deployment currently accepts these Candidate references:

- `a3s-code`, which also requires a non-empty `provider/model`;
- a relative local Adapter path beginning with `./` or `../`;
- a non-empty `oci://` Adapter reference.

Local and OCI Adapters are validated by Bench when deployed. Unsupported or
incomplete references cannot be added to the roster, saved over a roster
member, or deployed.

A visual Agent preset is not an executable Adapter. A3S Code defaults to J-50
with the locally available `anthropic/glm-5.2` route and a configured bundled Candidate. Codex defaults to F-35, Claude Code to
F-22, and both begin with empty Candidate/model fields until a real Adapter is
provided. Pilot branding and attire come from `pilotId`; they do not claim that
the Bench runtime can execute a visual preset.

The built-in airframe catalog contains J-50, J-35, F-35, F-22, and a neutral
Generic Test Prototype. Airframes are typed blueprints assembled from reusable
fuselage, surface, propulsion, cockpit, livery, pilot, and loadout modules. A
new procedural aircraft normally requires a blueprint and hangar catalog entry;
the generic prototype is available for custom Candidates that do not need a
dedicated visual model.

Effort is a visual axis from `none` through `xhigh` and controls mounted weapon
stores. Bench `run` has no effort argument, so effort never enters the scoring
request. The same rule applies to map-derived weather, Mach, angle of attack,
air density, turbulence, inspection rotation, pause, and streamline controls:
they affect only the 3D presentation and derived aerodynamic telemetry.

## 3D state and battle reports

The map workspace has two persistent surfaces: a dominant left-side Three.js
battlefield and a compact game-style mission board. The board presents Tasks as
a two-column, vertically scrollable sector selector instead of an unbounded
horizontal card strip. Flight telemetry is a
collapsed in-scene HUD that opens as a popover, not a permanent third column.
The scene renders the actual roster as a spaced multi-aircraft formation.

One shared deterministic `taskTheater` mapping keeps map cards, the selected
briefing, and the runtime exterior aligned. It resolves Tasks to eight
procedural theater families: training range, littoral front, mountain pass,
desert frontier, industrial city, arctic highland, forest valley, and offshore
platforms. Explicit built-in mappings are followed by category and stable-hash
fallbacks for future Tasks. Each theater owns recognizable terrain or water,
landmarks, palette, lighting, fog, clouds, and ambient motion. Task-derived
weather is a separate layer over the exterior, so selecting another map changes
the world as well as its weather. Ground and water materials use deterministic
procedural surface textures while remaining locally generated and disposable.
The battlefield contains no indoor chamber walls or ventilation fans.

The hangar pilot cards use distinct helmet, visor, suit, harness, color, and
brand-code portraits for A3S, Codex, Claude, and generic pilots. These portraits
are visual identity only; executable readiness continues to come from the
Candidate Adapter fields.

During a Campaign, each aircraft HUD is projected from its matching
`rosterEntryId` and shows authoritative `queued`, `starting`, `running`,
`completed`, `failed`, or `tracking_stopped` state. A score appears only when
that member's exact public result contains one.

The battle-report workspace preserves roster order and lists every Campaign
member with its status, Run ID, and real score when present. Selecting a
completed member loads that exact Run ID. Partial failure remains visible as
partial failure; the frontend does not synthesize an average score or fabricate
a combined `BenchRunResult`.

## Persistence and recovery

Browser metadata is stored in four separately validated, versioned records:

- `a3s-agent-evaluation.hangar.v1` stores the complete roster and active
  member;
- `a3s-agent-evaluation.campaign.v1` stores the Campaign's frozen Task and
  ordered sorties together with every member's Bridge tracking ID, exact Bench
  Run ID, result, status, and error;
- `a3s-agent-evaluation.single-run.v1` stores one single sortie's frozen Task,
  roster entry, exact input, Bridge tracking ID, Bench Run ID, result, and
  lifecycle state;
- `a3s-agent-evaluation.sorties.v1` stores immutable attribution snapshots
  keyed by exact Bench Run ID for later battle reports.

These records supplement rather than replace Bench's run journals and public
results. Invalid or incompatible browser records are ignored. Airframe, pilot,
callsign, and effort/loadout remain visualization and attribution metadata;
they do not override the Bench Task, result, or score.

A refresh cannot safely reattach to the bridge's in-memory polling relationship.
A nonterminal single sortie is therefore restored as `tracking_stopped`, with
its frozen Task/roster/input and known Bridge tracking ID/Bench Run ID intact. A
`running` Campaign is also restored as `tracking_stopped`; queued, starting,
and running members receive the same state, while already completed or failed
members remain intact. Submitted Bench processes may still continue.

When both a single-run record and a Campaign record are recoverable, the map
scene is restored from the evaluation with the later `startedAt`. Its frozen
Task, and the other recovered evaluation's Task, are added to the in-memory
catalog when absent so both retain their map identity. A later live catalog
refresh replaces matching snapshots with Bench data. A restored Task that is
absent from the live catalog remains identity-only and blocked from a new
deployment. A restored single-run card verifies terminal state through
`result <exact-run-id> --json`; it never falls back to the latest-result
endpoint for ownership.

In Task Lock mode the selected map is only a pre-run scene preview. At a
terminal result, the frontend requires Bench's real `task_id`, resolves that
Task, and rebinds the frozen sortie and map before saving attribution metadata.
If that identity cannot be verified, the run is not added to the sortie
archive.

## Stack and architecture

A3S智能体评测 follows the frontend architecture used by `apps/web`:

- Rsbuild and React 19;
- strict TypeScript;
- Valtio for shared serializable product state;
- Biome for formatting and linting;
- Vitest and Testing Library for frontend tests;
- Three.js for the WebGL scene;
- an app-local Node.js loopback adapter for the CLI boundary.

Feature code is grouped by product concern:

```text
src/
├── components/                 # Shell, mission board, in-scene telemetry, hover HUD
│   └── scene/                  # Aircraft, interaction, theaters, weather, flow
├── data/                       # Browse-only fallback Task catalog
├── features/
│   ├── bench/                  # Task detail, single/Campaign orchestration, manifests
│   ├── hangar/                 # Persistent 1–5 aircraft roster composition
│   ├── results/                # Exact-result lookup and formation debriefs
│   └── engineering/            # Doctor, validation, and lock workflows
├── lib/                        # Typed HTTP adapter, CLI display, aero model
├── state/                      # Shared serializable state and frozen snapshots
├── styles/                     # Semantic, concern-scoped CSS
└── types/                      # Bench and visualization contracts
scripts/
├── bench-bridge.mjs            # Loopback HTTP-to-CLI adapter
└── bench-arguments.mjs         # Shell-free CLI argument builders
```

The long-lived Three.js runtime owns its renderer, scene objects, animation
loop, observers, handlers, and GPU resources. Its battlefield environment owns
the active sky, lighting, fog, cloud bank, and procedural scenery stage. When
the resolved theater changes, the runtime swaps and disposes only that
theater-owned stage while retaining the aircraft fleet, interaction, wind,
weather, and renderer; final runtime disposal releases the remaining scene
resources.

See [Architecture](docs/ARCHITECTURE.md),
[Functional specification](docs/FUNCTIONAL_SPEC.md),
[Aircraft assets](docs/AIRCRAFT_ASSETS.md), and [Design system](DESIGN.md) for
the detailed contracts.

## Development

Install dependencies:

```sh
bun install
```

Build the local Bench component while developing this monorepo checkout:

```sh
cargo build --manifest-path ../../crates/bench/Cargo.toml --bin a3s-bench
```

Start the loopback bridge and Rsbuild frontend together:

```sh
just dev
```

The frontend opens on `http://127.0.0.1:3030`. The bridge listens on
`http://127.0.0.1:29655` and automatically prefers
`../../crates/bench/target/debug/a3s-bench`, then the release binary, then the
installed `a3s bench` component.

Use environment variables when another local binary or Bench working directory
is required:

```sh
A3S_BENCH_BIN=/absolute/path/to/a3s-bench \
A3S_BENCH_CWD=/absolute/path/to/project \
just dev
```

The working directory controls where project-local `.a3s/config.acl` and
`.a3s/bench/` state are discovered. The bridge refuses non-loopback bind
addresses.

## Validation

```sh
bun run format:check
bun run lint:check
bun run typecheck
bun run test
bun run build
```

The production frontend is written to `dist/windhole`. A static deployment by
itself is browse-only: live deployment still requires the local bridge, a
reachable Bench component, and a Doctor-ready Runtime.
