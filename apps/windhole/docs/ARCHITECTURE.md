# A3S智能体评测 Architecture

## System boundary

A3S智能体评测 is a local control surface for A3S Bench, not a second benchmark
runtime. Bench remains authoritative for Task validation, Runtime selection,
run journals, Judges, scores, and Run IDs. The frontend owns roster composition,
deployment snapshots, presentation state, and browser-side attribution metadata.

```text
React presentation
       │
feature controllers ── Valtio serializable state
       │
typed /api/v1/bench adapter
       │
loopback Bench bridge
       │  spawn(command, argument[], { shell: false })
       ▼
a3s-bench component ── .a3s/config.acl + .a3s/bench/
```

The browser receives public CLI projections. It does not read configuration
secrets, private Judge diagnostics, Candidate source paths, or Bench result
files directly. A successful health request is not sufficient for deployment:
Doctor must also report a non-empty Runtime provider/detail and
`runtime.ready=true`.

There is no disconnected execution substitute. The browse-only fallback Task
catalog and Three.js scene can render when the bridge is unavailable. Only the
live bridge can create a local tracking ID and launch a process; only Bench
output can supply a Run ID, result, or score.

## Data authority and handoff

| Data | Authority | Consumer |
| --- | --- | --- |
| Task catalog and Task detail | A3S Bench | map catalog and deployment preflight |
| Candidate and model | versioned hangar roster or explicit Candidate Lock | ordinary or locked Bench input |
| Airframe, pilot, effort/loadout, callsign | versioned hangar roster | 3D identity and report attribution metadata only |
| Task Lock and Candidate Lock paths | explicit single-sortie lock controls | locked run input only |
| Bridge tracking ID (`windhole-*`) and process status | loopback bridge | single/Campaign polling |
| Run ID and public result | A3S Bench | HUD terminal state and battle reports |
| Weather, flight controls, visual loadout | frontend visualization | Three.js and derived telemetry only |

For ordinary runs, `HangarRosterEntry` is the sole configuration record for
aircraft identity and executable Candidate/model input. Map-side Candidate,
model, and effort changes update the active roster member; there is no parallel
run-configuration copy to drift out of sync. The selected Task ID is the map.

At launch, the frontend creates an immutable snapshot:

- `RunSortieSnapshot` freezes one Task, one roster member, and the exact normal
  or locked `StartBenchRunInput`;
- `RunCampaignSnapshot` freezes one Task and every roster member in roster
  order, with a distinct unlocked input for each member.

Snapshots are used for both execution and later report attribution. Mutable UI
state is never consulted to reinterpret an in-flight job.

## Frontend layers

### Presentation and Three.js runtime

`src/components/` owns the map scene, game-style map catalog integration,
in-scene telemetry HUD, and stable product shell. The desktop map workspace has
two persistent surfaces: a dominant Three.js battlefield and a compact mission
board whose bounded two-column sector selector scrolls vertically. Telemetry is
a collapsed battlefield overlay that opens a transient popover, not a third
layout column. Feature workspaces remain under their owning concerns:

- `features/hangar/components/` owns formation composition;
- `features/results/components/` owns single and Campaign debriefs;
- `features/engineering/components/` owns Doctor, validation, and lock actions.

Components render state and emit intent. They do not call `fetch`, spawn
processes, or interpret CLI envelopes.

The long-lived Three.js runtime stays below the React presentation boundary:

```text
wind-tunnel-scene.tsx
       │ buildRosterFormation(hangar.roster)
       ▼
scene/wind-tunnel-runtime.ts
       ├── flight-formation.ts ─ roster → 1–5 spaced perspective slots
       ├── pilot-profile.ts ─── pilotId → attire, markings, cockpit, brand family
       ├── weapon-loadout.ts ── visual effort → mounted stores
       ├── aircraft-fleet.ts ── slot → Task pose → inspection transform → model
       ├── aircraft-registry.ts ─ visual family + explicit airframe → instance
       │    └── procedural-fighter.ts
       │         └── aircraft-assembler.ts
       │              ├── aircraft-blueprints.ts ─ J-50, J-35, F-35, F-22, prototype
       │              ├── aircraft-module-fuselage.ts
       │              ├── aircraft-module-surfaces.ts
       │              ├── aircraft-module-propulsion.ts
       │              ├── aircraft-module-cockpit.ts
       │              └── aircraft-livery.ts + weapon-loadout-visual.ts
       ├── aircraft-interaction.ts ─ Raycaster pointer/touch/keyboard controls
       ├── campaign-aircraft-hud.ts ─ rosterEntryId → real member state
       ├── battlefield-theater.ts
       │      └── shared taskTheater resolver → one of eight deterministic profiles
       ├── battlefield-environment.ts
       │      └── owned sky, lights, fog, cloud bank, and replaceable stage
       │           └── battlefield-scenery.ts + battlefield-scenery-elements.ts
       │                ├── range, coast, mountains, desert, city, arctic,
       │                │   forest, or offshore procedural exterior
       │                └── disposable deterministic DataTexture surfaces
       ├── task-weather.ts ───── selected Task → deterministic weather overlay
       ├── weather-system.ts ─── rain, hail, crosswind, fog, lightning
       ├── wind-field.ts ─────── 3D particles and streamlines
       └── aircraft-spotlight.ts ─ focused-light selection feedback

features/hangar/hangar-configuration.ts
       │ serializable draft + validated roster entry
       ▼
scene/hangar-preview-runtime.ts
       └── fixed-camera, Box3-normalized single-aircraft preview
```

React never stores `THREE.Object3D` in Valtio. Each runtime owns its renderer,
scene objects, RAF, observers, input handlers, geometries, materials, and
textures. React owns serializable roster entries, selection, Bench state, and
visual parameters. Formation changes replace the aircraft fleet without
recreating the WebGL renderer.

`battlefield-theater.ts` is the presentation source of truth for Task-to-theater
mapping. Its `taskTheater` resolver is shared by map cards and the selected-map
briefing, while the runtime uses the same resolver to construct the exterior.
Eight explicit built-in Task mappings are backed by category mappings and a
stable Task-ID hash for future Tasks, so a Task retains a deterministic
exterior without adding renderer branches. Weather resolution remains separate
and is composited over the chosen theater.

The runtime owns the battlefield environment for its full lifetime. When the
resolved theater changes, it creates the next stage, applies its atmosphere,
removes the old stage, and disposes the old stage's geometries, materials, and
textures while preserving the renderer, aircraft fleet, interaction
controller, wind field, and weather system. Final runtime disposal removes the
active battlefield and releases all remaining scene and renderer resources.
There are no indoor chamber-wall or fan objects in this scene graph.

Airframes are data-driven blueprints rather than renderer branches. The five
built-ins are J-50, J-35, F-35, F-22, and Generic Test Prototype. Pilot visual
identity is derived from `pilotId`, independently of the executable Candidate
reference. This keeps Codex and Claude branding visible even while their
Candidate fields are empty or point to custom Adapters.

The scene always builds from the current roster. During an active Campaign, a
click changes only the locally inspected aircraft; it cannot switch the active
roster member or mutate the frozen Campaign. HUD lookup joins the hovered
aircraft instance ID to `LabCampaignMemberRun.rosterEntryId`, producing real
queued, starting, running, completed, failed, or tracking-stopped state. A
completed HUD displays a score only if that member's exact result supplied one.

Weather, Mach, angle of attack, air density, turbulence, inspection rotation,
pause, and flow rendering are deliberately outside `StartBenchRunInput`.
Effort selects visible weapon stores and is also excluded. These values cannot
overwrite Judge metrics or masquerade as Bench progress.

### Application controllers

Each workflow has a focused controller:

- `useBenchController` loads health, Doctor, the Task catalog/detail, runs one
  frozen sortie, and polls its bridge-local process;
- `campaign-controller.ts` freezes the complete roster, schedules at most two
  real sorties concurrently, polls each independent bridge process, and
  resolves each exact Run ID;
- `useHangarController` validates, adds, updates, activates, and removes roster
  members;
- `useResultController` resolves an explicit Run ID, the explicit latest-result
  action, or the current single-sortie result;
- `useEngineeringController` owns Doctor, Task validation, and both lock
  workflows with operation-local loading and error state.

Controllers call the typed adapter in `src/lib/api.ts` and update Valtio state.
They do not generate offline run progress or scores.

### Shared state and run locks

`src/state/lab-state.ts` contains serializable product state and the frozen
snapshot factories. A single evaluation lock is derived from both execution
domains:

```text
single stage is nonterminal OR Campaign status is running
                         │
                         ▼
                 evaluation is active
```

While active, controllers reject Task/map changes, connection refreshes,
hangar mutation, active-member switches, Candidate/model/effort changes,
deployment-scope changes, and lock changes. The render-only inspection pivot is
not part of this lock.

### Candidate deployment contract

Normal mode accepts only:

- the bundled `a3s-code` Adapter with a non-empty `provider/model`;
- relative local Adapter references beginning with `./` or `../`;
- non-empty `oci://` references.

Local and OCI contents are validated by Bench at launch. Codex and Claude are
visual pilot presets, not implicit executable Candidate names. Unsupported or
incomplete entries fail roster and deployment validation.

Locked mode passes a Task Lock path and Candidate Lock path, omits `model`, and
sets `locked=true`. It is supported only for a single sortie. Campaign
preflight rejects lock mode rather than duplicating one locked Candidate across
several visually distinct roster members.

## Real execution lifecycles

### Single sortie

1. The user selects a ready Task as a map and activates a hangar member.
2. Live connection, Doctor readiness, Task availability, and Candidate input are
   checked. Locked mode checks explicit lock paths and leaves lock validation to
   Bench; its currently selected map is presentation preview only.
3. The frontend freezes `RunSortieSnapshot` and posts its exact input.
4. The bridge creates one in-memory `windhole-*` tracking ID and starts one
   shell-free Bench process. This local ID is not issued by Bench.
5. The frontend polls only that Bridge tracking ID.
6. When the process returns a Run ID, the frontend calls
   `GET /api/v1/bench/results/:runId` and rejects a mismatched response.
7. Any ordinary result `task_id` must match the frozen Task, and a completed
   ordinary result must supply it. For Task Lock, the controller resolves the
   real `task_id` returned by Bench, rebuilds the sortie against that Task,
   selects the resolved map, and adds the Task to the in-memory catalog when
   necessary.
8. The complete public result becomes the score authority, and the verified
   sortie snapshot is saved under that real Run ID. A missing, mismatched, or
   unresolvable Task identity fails closed and is not archived.

### Campaign

1. The user selects Campaign scope. Preflight validates the selected Task,
   Doctor, unlocked mode, and every roster member.
2. The frontend freezes one Task and the complete ordered roster. Every member
   begins as `queued` with its own input.
3. Two workers at most transition members through `starting` and `running`.
   Additional members stay queued until a worker returns.
4. Every member owns a distinct Bridge tracking ID. Missing or duplicate
   tracking IDs fail only that member.
5. Every completed process must return a distinct Bench Run ID. The controller
   then calls `/results/:runId`; missing, duplicate, or mismatched IDs fail that
   member.
6. A member failure does not stop other workers. The terminal Campaign status
   is `completed`, `completed_with_failures`, or `failed` according to real
   member outcomes.
7. The formation debrief lists members in frozen roster order and opens only
   the selected member's exact Run ID. It never uses `latest` for attribution
   and never creates an aggregate score record.

Bench currently emits its JSON run projection at process completion. The
bridge therefore maps only actual process state: an active child is
reported generically as `running`, a successful child is `completed`, and an
unsuccessful child is `failed`. It does not invent intermediate CLI events or
claim a more specific phase than the CLI exposes.

## Persistence and refresh semantics

Four versioned browser stores provide validated attribution and recovery
metadata:

```text
a3s-agent-evaluation.hangar.v1
  └── full roster + active roster entry

a3s-agent-evaluation.campaign.v1
  └── frozen Task + ordered sorties + per-member Bridge tracking ID/Run/result/status/error

a3s-agent-evaluation.single-run.v1
  └── frozen Task/roster/input + Bridge tracking ID/exact Run/result/lifecycle/timestamps/error

a3s-agent-evaluation.sorties.v1
  └── up to 100 exact Run ID → frozen single-sortie snapshots
```

Startup restores the hangar first, then independently parses the Campaign and
single-run records before choosing scene ownership. If both evaluations are
recoverable, the one with the later `startedAt` supplies the selected map.
Each recovered evaluation's frozen Task is merged into the in-memory catalog
when absent, preserving map name, weather, pose, and attribution. Restore call
order is not used as an implicit priority.

On a later live catalog refresh, Bench's current record replaces a restored
snapshot with the same Task ID. A recovered Task absent from the live response
stays in the catalog with blocked availability and an identity-only recovery
reason. It can continue to label the restored scene and report, but cannot be
used to start a new deployment.

Parsers enforce versions, bounded strings, enumerated values, unique roster,
Bridge tracking, and Bench Run IDs, member/result consistency, timestamp
consistency, and archive size limits. Invalid records are ignored; persistence
failure never replaces the authoritative Bench result. The sortie archive
binds one exact Run ID to one immutable attribution snapshot and rejects
ambiguous ownership.

The Bridge tracking registry is process memory, so a browser refresh cannot
prove it is still polling the same active process. A persisted nonterminal single run is
therefore restored as `tracking_stopped` while preserving its frozen
Task/roster/input and known Bridge tracking ID/Bench Run ID. A persisted
`running` Campaign is also restored as `tracking_stopped`; members that were
queued, starting, or running receive the same state, while terminal members and
their exact results remain intact. Submitted Bench processes may continue, and
the UI does not claim they were cancelled or completed.

The restored single-run card uses its exact Run ID to call
`GET /api/v1/bench/results/:runId`. A matching terminal projection reconciles
and persists the single-run state. It does not call the latest-result route as
an ownership fallback. For a restored Task Lock sortie, reconciliation first
resolves Bench's terminal `task_id`, rebinds the frozen Task and map, and only
then writes the sortie archive; unverifiable Task identity remains unarchived.

These stores carry frontend attribution, not execution truth. In particular,
airframe, pilot, callsign, and effort/loadout are visualization metadata. Bench
remains authoritative for Task identity, Run state, terminal results, and
scores; the bridge alone owns its local tracking IDs and child-process state.

## API mapping

| HTTP route | CLI operation | Mutation |
| --- | --- | --- |
| `GET /api/v1/bench/health` | `--component-info --json` | No |
| `GET /api/v1/bench/tasks` | `list [--all] --json` | No |
| `GET /api/v1/bench/tasks/:task` | `info <task> [--all] --json` | No |
| `POST /api/v1/bench/runs` | `run … --json` | Yes: Bench state and workload |
| `GET /api/v1/bench/runs/:job` | in-memory Bridge tracking projection | No |
| `GET /api/v1/bench/results/latest` | `result --json` | No |
| `GET /api/v1/bench/results/:run` | `result <run-id> --json` | No |
| `POST /api/v1/bench/doctor` | `advanced doctor --json` | No |
| `POST /api/v1/bench/tasks/check` | `advanced check <source>` | No |
| `POST /api/v1/bench/locks/task` | `advanced task lock …` | Yes: requested output file |
| `POST /api/v1/bench/locks/candidate` | `advanced candidate lock …` | Yes: requested output file |

All responses use the repository REST envelope with HTTP `code`, `message`,
`data`, `requestId`, and `timestamp`. Errors add a stable business
`statusCode` and details.

## Security and failure behavior

- The bridge binds only to `127.0.0.1`, `::1`, or `localhost`.
- Requests with a browser `Origin` are accepted only from the exact configured
  Windhole or bridge loopback origin. Missing origins remain valid for local
  CLI and test clients; `null`, lookalike localhost domains, and remote origins
  are rejected.
- Every POST route requires the `application/json` media type before its body
  is read.
- Every route that invokes the CLI first verifies `--component-info --json`
  reports `component=bench` and `cli_protocol=a3s-bench-cli/v1`. JSON command
  projections must use the `a3s.bench.output.v1` envelope.
- Child processes use `shell: false` and validated argument arrays.
- Run requests reject extra fields, require an explicit boolean `locked`,
  reject an empty model, and reject `locked + model`.
- Request bodies and process output have explicit size limits.
- The Campaign scheduler submits no more than two members concurrently, and the
  bridge independently rejects a third active run.
- Missing bridge, failed health, failed Doctor, non-ready Runtime, blocked Task,
  missing Judge configuration, invalid Candidate, run failure, and result
  mismatch remain distinct user-visible failures.
- A member failure stays attached to that roster member and does not become a
  score or abort unrelated Campaign members.
- A failed single run preserves a Run ID only from the anchored Bench terminal
  form `run local-* failed: ...`; unanchored lookalike text cannot populate the
  public tracking record.
- Lock output paths remain explicit user inputs; the bridge never chooses a
  destination silently.
