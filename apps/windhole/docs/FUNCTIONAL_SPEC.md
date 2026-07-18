# A3S智能体评测 Functional Specification

## Primary journeys

```text
Connect to local Bench + pass Doctor
→ build and save aircraft/pilot combinations in the hangar
→ choose a Bench Task as the map
→ choose single-sortie or Campaign scope
→ deploy real Bench job(s)
→ follow per-aircraft state in 3D
→ open exact Run ID battle report(s)
```

The hangar and map are one workflow, not two independent configuration forms.
The hangar roster owns each ordinary deployment combination; the map chooses the
Task and deployment scope. The UI must never replace unavailable execution with
simulated progress, a local fake Run ID, or a generated score.

## Map selection

- Load the complete Bench Task catalog, including ready and blocked Tasks.
- Present every Task as a selectable map.
- Default to ready maps and reveal blocked maps through an explicit filter.
- Search by Task name, ID, or category.
- Fetch `info` when a live Task is selected and show availability, admission,
  provenance, and the optional description through progressive disclosure.
- Treat the selected Task ID as the execution input and as the deterministic
  source for the scene's exterior theater, visual weather, and task-specific
  aircraft pose.
- Use one shared deterministic Task-to-theater resolver for map cards, the
  selected-map briefing, and the Three.js runtime. Explicit built-in mappings,
  category fallbacks, and a stable Task-ID hash must make future Tasks
  extensible without ad hoc renderer conditions.
- Present map choice as a compact game-style mission route. Keep search,
  blocked-map filtering, exact Task identity, and advanced deployment inputs
  behind progressive disclosures.
- Reject map changes while a single sortie or Campaign is active.
- A Task requiring `judge_model` is not deployable until Doctor reports one.

## Hangar formation

- Maintain an ordered roster of one to five entries.
- Give every entry a stable unique ID and explicit airframe, pilot identity,
  Candidate, model, effort, callsign, and visual loadout.
- Provide J-50, J-35, F-35, F-22, and Generic Test Prototype airframes.
- Provide A3S Code, Codex, Claude Code, and generic pilot identities with
  distinct attire, helmet markings, cockpit accents, and brand colors.
- Source the map's active ordinary Candidate/model and the 3D formation from
  this same roster. Map-side Candidate/model/effort edits update the active
  member instead of creating an override object.
- Allow users to add a valid draft, update the active member, select a member,
  remove a member while keeping at least one, and enter the map with the active
  combination.
- Reject all hangar mutations while an evaluation is active.
- Persist the complete roster and active member in a validated versioned
  browser record.

Pilot identity and executable Candidate are independent. A3S Code with its
default GLM-5.2 route defaults to J-50 and the bundled `a3s-code` Adapter. Codex
defaults to F-35 and Claude Code to F-22, but the latter two are visual presets
with empty Candidate/model fields until the user supplies a real Adapter.
Selecting a branded pilot must not pretend that Bench can execute a Candidate
of the same brand.

## Candidate preflight

Normal deployment supports only:

- `a3s-code` with a non-empty `provider/model`;
- a relative local Adapter path beginning with `./` or `../`;
- a non-empty `oci://` Adapter reference.

Incomplete and unsupported references cannot be added, saved, or deployed.
The frontend validates the reference shape; Bench remains responsible for
validating local or OCI Adapter contents at execution time.

A deployment also requires:

- a live bridge health result;
- a successful Doctor projection with `runtime.ready=true` and non-empty
  Runtime provider/detail;
- a selected Task that passes Task-specific readiness rules;
- a valid active member for a single sortie, or valid Candidate/model input for
  every roster member in a Campaign.

The browse-only fallback state permits Task and 3D inspection but never enables
deployment.

## Single-sortie deployment

- Freeze the selected Task, active roster entry, and exact run input before
  posting to the bridge.
- Support normal Candidate/model input.
- Support an explicit locked mode using separate Task Lock and Candidate Lock
  paths, `locked=true`, and no model field.
- In locked mode, label the selected map and derived weather as a scene preview,
  not as the proven identity of the locked Task.
- Poll only the Bridge tracking ID returned for that sortie. The `windhole-*`
  value is a local process-correlation ID, not a Bench-issued Job ID.
- When the tracked process completes, require a non-empty Bench Run ID and load
  `/api/v1/bench/results/:runId`.
- Reject a public result whose `run_id` differs from the requested Run ID.
- For an ordinary result, reject any supplied `task_id` that differs from the
  frozen Task, and require a completed result to supply the matching ID. For a
  Task Lock terminal result, resolve the real Bench `task_id` and rebuild the
  frozen sortie with that Task before archival. If the Task ID is missing,
  mismatched, or cannot be resolved, fail closed and do not archive the sortie.
- Show a score only from that exact public result.
- Save a frozen sortie manifest under the real Run ID for later attribution.
- Prevent a second launch and all configuration mutation until the run reaches
  `completed` or `failed`.

## Campaign deployment

- Offer a separate Campaign scope that deploys the complete current roster
  against one selected map.
- Reject Campaign scope in lock mode. Lock files are single-sortie only.
- Freeze the Task and the complete ordered roster before starting the first
  member.
- Create one independent input and runtime record for every member, including
  its status, Bridge tracking ID, Bench Run ID, result, timestamps, and error.
- Start with every member `queued`.
- Execute at most two real Bench jobs concurrently. Members beyond the active
  pair remain queued and visibly report that they are waiting for a slot.
- Transition each member through actual `starting`, `running`, and terminal
  state; do not estimate progress from elapsed time.
- Continue other queued or running members when one member fails.
- Reject missing or duplicate Bridge tracking IDs, missing or duplicate Bench
  Run IDs, and Run ID mismatches on the affected member.
- Resolve each completed tracked process through its own `/results/:runId`
  request. Never use the latest-result endpoint to infer Campaign ownership.
- End as `completed` when all members complete,
  `completed_with_failures` when completion and failure are mixed, or `failed`
  when no member returns a valid result.
- Do not synthesize a formation average or a combined `BenchRunResult` for a
  partially failed Campaign.

Stopping frontend tracking is not Bench process cancellation. Members without
a terminal result become `tracking_stopped`, and the UI explains that already
submitted Jobs may continue.

## 3D evaluation scene

The map workspace must remain a two-surface composition: a dominant, real
Three.js perspective battlefield built from the current hangar roster and a
compact mission board. Telemetry is collapsed into an in-scene HUD that opens
on demand; it must not consume a third persistent column. The battlefield
includes a spaced multi-aircraft formation, a procedural exterior, volume
particles, streamlines, aircraft body hit testing, and visual weather. It must
not render indoor chamber walls or ventilation fans.

- Provide eight deterministic procedural exterior families: training range,
  littoral front, mountain pass, desert frontier, industrial city, arctic
  highland, forest valley, and offshore platforms.
- Give each exterior recognizable terrain or water, landmarks, palette,
  lighting, fog, ambient motion, and subtle procedural surface detail so
  changing maps is visually more substantial than changing a weather label.
- Present Tasks as a bounded two-column sector selector with vertical scrolling;
  do not put the complete Catalog in one unbounded horizontal card strip.
- Layer Task-derived weather over the selected exterior. Exterior selection
  and weather selection are related presentation inputs, not one combined
  preset.
- On a theater change, preserve the renderer, aircraft fleet, interaction,
  wind field, and weather system; replace only theater-owned scene resources
  and dispose the removed geometries, materials, and textures.

- Render one scene aircraft per roster entry and use the roster entry ID as the
  stable instance identity.
- Use the explicit roster airframe rather than inferring all aircraft from one
  shared model string.
- Use `pilotId` for pilot attire and aircraft brand treatment, independently of
  the executable Adapter reference.
- Give every built-in pilot a visibly distinct portrait treatment (helmet,
  visor, suit/harness, brand color, and short identity code) in the hangar.
- Use effort for mounted weapons only.
- Let users select an aircraft on its body, rotate its inspection pivot by
  pointer/touch drag, switch with numeric keys, and reset its view.
- Indicate selection with a focused light and labels. Do not add a floor ring,
  direction arrows, or self-illuminated body material.
- While a Campaign is running, clicking another aircraft changes only local
  inspection and does not switch the active roster entry.
- Join a hovered Campaign aircraft to its member by `rosterEntryId` and show
  real queued, starting, running, completed, failed, or tracking-stopped state.
- Show a completed member's score only when its exact result contains one.
- On Task change, reset manual inspection, switch to the resolved exterior, and
  apply clear, light rain, moderate rain, heavy rain, hail, typhoon,
  thunderstorm, or compound visual weather.

Mach, angle of attack, air density, turbulence, weather, effort/loadout,
inspection transforms, pause, and streamline visibility are presentation and
derived-telemetry inputs only. None is sent to Bench or used to alter the Judge
score.

## Battle reports

- Read a specific `local-*` Run ID.
- Provide a separate user action to read the latest local result for general
  archive browsing.
- Reopen the current single-sortie result by its exact Run ID.
- Keep a restored single-run card available when its exact Run ID and frozen
  sortie are known but no terminal result is cached. Activating that card must
  query the exact Run ID, reconcile the restored lifecycle when Bench returns
  a terminal result, and never substitute the latest result.
- Show the current Campaign as an ordered formation debrief with each member's
  status, Run ID, score when present, and failure detail.
- Enable a completed Campaign member to open only its own Run ID.
- Present score, primary metric, Task, Runtime, model, governance status,
  identities, lock digests, result digest, and public model usage when present.
- Attribute a report to the frozen map, weather visualization, aircraft,
  callsign, pilot, actual Candidate or lock input, model, and effort/loadout
  metadata saved for that Run ID.
- Render in-progress and failed public projections without inventing a score or
  exposing private diagnostics.
- Keep loading, empty, error, nonterminal, failed, and completed states local to
  the report workspace.
- Explain that public projections omit private diagnostics and Candidate source
  paths.

The latest-result endpoint is a manual archive convenience only. Campaign
controllers and Campaign report buttons must always use explicit Run IDs.

## Persistence and refresh

- Maintain four independent versioned browser records:
  - hangar: complete roster and active member;
  - Campaign: frozen Task, ordered sorties, and each member's Bridge tracking
    ID, exact Bench Run ID, result, status, timestamps, and error;
  - single-run: frozen Task, roster entry, exact input, Bridge tracking ID,
    exact Bench Run ID, result, lifecycle state, timestamps, and error;
  - sortie archive: immutable frozen Task/roster/input attribution keyed by
    exact Run ID.
- Restore the hangar before evaluation records. Restore both the Campaign and
  single-run records, then use the recoverable evaluation with the later
  `startedAt` as the map/scene owner. Do not let restore call order decide the
  selected map.
- Add each recoverable evaluation's frozen Task to the in-memory catalog when
  absent so map name, weather, pose, and report attribution retain the frozen
  identity. On live catalog refresh, prefer Bench's current record for a
  matching Task ID. Retain a restored Task missing from the live catalog only
  as a blocked, identity-only entry that cannot start a new deployment.
- Persist each completed single sortie or Campaign member's immutable sortie
  snapshot under its real Run ID.
- Validate versions, record shapes, bounded strings, enum values, unique IDs,
  member-to-snapshot alignment, and result-to-Run-ID alignment before accepting
  browser records.
- Ignore malformed or incompatible metadata rather than presenting it as Bench
  state.
- Preserve a single-run terminal result across refresh. Convert any restored
  nonterminal single run to `tracking_stopped` without discarding its frozen
  Task/roster/input, Bridge tracking ID, or Bench Run ID.
- Preserve completed and failed Campaign members across refresh.
- Convert a restored `running` Campaign to `tracking_stopped`; likewise convert
  its queued, starting, and running members. A refresh cannot safely resume the
  bridge's in-memory polling relationship.
- Reconcile a restored single run only by its exact Run ID. The explicit
  latest-result action remains available for archive browsing but is not a
  recovery or ownership mechanism.
- Apply the same Task Lock terminal identity check during restored exact-result
  reconciliation: use Bench's real `task_id` to rebind the frozen Task and map,
  or refuse archival when it cannot be verified.
- Never claim that `tracking_stopped` means a submitted Bench Job was cancelled.

Browser persistence is attribution metadata. Bench run journals and public
results remain authoritative if browser storage is unavailable. Airframe,
pilot, callsign, effort, loadout, map weather, and pose are visualization or
attribution metadata and never supersede Bench's Task identity, lifecycle,
result, or score.

## Engineering tools

- Run Doctor and show Runtime provider, readiness detail, and Judge model.
- Validate a local TaskBundle without writing Bench state.
- Create a Task Lock from a built-in Task ID or local Task source.
- Create a Candidate Lock with an optional model.
- Prime the corresponding single-sortie locked input after a lock is created.
- Keep all input labels persistent and reveal exact advanced commands only on
  request.
- Disable file-producing operations outside the live bridge boundary.
- Report each outcome beside its owning operation.

## Connection and execution states

| State | Required behavior |
| --- | --- |
| Checking | Preserve the final layout, show bounded connection progress, and disable deployment |
| Live and Doctor ready | Load real Tasks and permit operations that pass Task/Candidate preflight |
| Browse-only fallback | Show fallback Tasks and 3D controls, but create no jobs, runs, locks, progress, or scores |
| Command failed | Preserve inputs and attach the CLI error to the owning workflow |
| Campaign member failed | Continue unrelated members and preserve the member-specific error |
| Tracking stopped | Preserve known terminal results and state that submitted Jobs may still run |
| Bridge unavailable | Keep non-execution visualization usable and offer connection recheck |

## Accessibility

- Every icon-only control has an accessible name.
- Aircraft selection uses focused lighting plus textual identity, not color
  alone.
- Every operational status has text in addition to semantic color.
- Inputs have persistent labels and visible focus.
- Reduced-motion preferences slow the WebGL field and suppress nonessential CSS
  motion.
- Aircraft inspection and supported workspace controls remain keyboard
  operable at the supported desktop width.
