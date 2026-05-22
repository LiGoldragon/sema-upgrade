# sema-upgrade Architecture

`sema-upgrade` is the runtime side of the Sema schema-upgrade component.
It owns migration modules, the module index compiled into a given build,
and the execution path for ordinary upgrade attempts.

In the version-handover stack `sema-upgrade` plays two roles. First, it
is the **protocol witness**: `src/handover.rs` carries a testable
state-machine implementation of the `signal-version-handover` protocol
that production daemons can model their wiring after. Second, it is
the **end-to-end sandbox host**: Nix-owned apps in this repo prove the
full migration + selector flip + cross-daemon write isolation slice
against the live database without touching production state. The crate
remains library-shaped today; a future `sema-upgrade-daemon` will own
the persona engine's view of upgrade orchestration once the daemon
shape settles.

## Constraints

- The runtime uses `signal-sema-upgrade` for ordinary peer requests.
- Owner policy uses `owner-signal-sema-upgrade`; the prototype defines
  the contract but does not yet persist policy.
- The runtime uses `signal-executor`; upgrade operations lower to
  component-local commands, not payload-bearing Sema operations.
- Component-local commands project into `signal-sema` operation and
  outcome classifications for observation.
- Supported migrations are selected from a compile-time module index.
- Database migrations write into a new target database path; the
  migration code never rewrites the source database in place.
- Atomic handover prototypes use `signal-version-handover` for private
  upgrade messages and `version-projection` for per-type forward/reverse
  representation checks.
- Live migration smoke tests run through Nix apps or checks. The operator
  does not hand-run the copy/migrate/daemon/CLI sequence as an ad-hoc shell
  script.
- Unsupported component/version pairs return `UpgradeRejected` through
  the contract reply, not a frame-level rejection.

## Prototype Module Index

The first module is:

```text
src/migrations/persona_spirit/version_0_1_0_to_0_1_1.rs
```

The Rust module name is identifier-safe; the public contract still names
the version range as `(0 1 0)` to `(0 1 1)`.

### Two-submodule migration pattern

Every migration module under `src/migrations/<component>/version_X_Y_Z_to_X_Y_W.rs`
follows the same two-submodule shape:

- `mod historical` — **private reproduction** of the deployed old types,
  copied byte-for-byte from the old signal contract. Each module owns
  its own historical layout. Includes every type the historical record
  depends on, even primitives that have not changed shape — rkyv
  archived layout depends on the closure of dependent types, so the
  layout has to be reproduced as a closed bundle.
- `mod current_shape` — overrides **only the fields that changed**,
  re-using the current public contract types for everything else.
  `From<historical::X> for current_shape::X` composes a conversion
  chain that the module's `run` function invokes per record.

This pattern keeps each migration module self-contained: nothing breaks
the migration when the current contract evolves further, and historical
layouts stay frozen alongside the records they decode. Skill reference:
`skills/spirit-cli.md` §"Substrate migration discipline".

### First module — Spirit Certainty → Magnitude

The Spirit module carries two shapes:

- historical private storage wrappers matching the deployed
  `persona-spirit` `v0.1.0` database bytes;
- current storage wrappers matching the next Spirit store shape where
  `Entry.certainty` is `signal-sema::Magnitude`.

The migration reads every historical record from the `records` table,
maps `Certainty::Maximum | Medium | Minimum` into
`Magnitude::Maximum | Medium | Minimum`, preserves identifiers and
daemon-stamped date/time, and asserts the converted records into a fresh
target database.

## Handover Prototype

`src/handover.rs` is the first testable shape for the private upgrade
socket protocol. It is intentionally not the production Spirit daemon
wiring. The module models two versions of the same component:

- the current endpoint starts as public;
- the next endpoint is prepared from a copied/migrated database;
- `AskHandoverMarker` reads the current endpoint's schema and write marker;
- `ReadyToHandover` is accepted only if the marker has not moved;
- `HandoverCompleted` makes the current endpoint private-upgrade-only and
  makes the next endpoint public;
- mirrored writes use `VersionProjection<Source, Target>`;
- writes that cannot be projected record typed `Divergence` payloads instead
  of being silently dropped.

The handover prototype gives the codebase a concrete place to test the
protocol before the production daemon grows private upgrade sockets, active
socket selection, and high-water-mark replay.

## Temporary CLI

`sema-upgrade-temporary` is not the final daemon client. It is a
one-argument migration utility for the first live upgrade. Its input is:

```text
(Attempt (<source-path> <target-path> (<component> <source-version> <target-version>)))
```

For the Spirit upgrade:

```text
(Attempt (/tmp/persona-spirit-v0.1.0.redb /tmp/persona-spirit-v0.1.1.redb (persona-spirit (0 1 0) (0 1 1))))
```

The CLI routes through the same compiled `MigrationIndex` as the runtime
engine, then prints `UpgradeCompleted` or `UpgradeRejected` as NOTA.

## Nix Live Sandbox

The flake exposes `.#spirit-migration-sandbox` as the live smoke-test
surface for the first Spirit migration. It takes one source database path,
copies it into a temporary directory, runs `sema-upgrade-temporary`, starts
the tagged `persona-spirit` `v0.1.1` daemon on temporary ordinary and owner
sockets, and uses the tagged `spirit` CLI against those sockets.

The sandbox proves four things without touching the live database:

- the current live `v0.1.0` database copy can be decoded by the historical
  migration shape;
- the migration produces a `v0.1.1` redb readable by the current Spirit
  daemon;
- the current Spirit CLI can observe migrated records through the daemon;
- the widened `Magnitude::High` value can be written and queried on the
  migrated copy.

`.#spirit-smart-handover-sandbox` is the stronger two-version witness. It
takes the current `v0.1.0` database path, copies it into an isolated sandbox,
starts a tagged `v0.1.0` daemon against the copy, writes a legacy-compatible
record before snapshot, migrates the snapshot to `v0.1.1`, starts a tagged
`v0.1.1` daemon against the migrated copy, runs the
`signal-version-handover` protocol prototype, switches the sandbox's public
selector from current to next, writes a `High` record through the next daemon,
and verifies the current database did not receive that next-only write.

This is still a sandbox witness, not the full production private-upgrade
socket wiring. The next `persona-spirit` daemon version now owns a
`signal-version-handover` upgrade socket for marker, readiness, and completion
checks. The deployed `v0.1.0` daemon does not yet own that socket, so the app
still runs the cross-version protocol through `sema-upgrade-handover-temporary`
while the two real Spirit daemon versions prove the database and CLI sides.

The flake also exposes `.#spirit-migration-stage` for the first persistent
cutover. It takes a source `v0.1.0` database path and a target `v0.1.1`
database path. The target daemon must be stopped by the caller before the app
runs. The app migrates into a same-directory temporary database, verifies a
copy of that database through a tagged `v0.1.1` daemon and CLI, backs up the
previous target database if one exists, and atomically renames the unmodified
staged database into the target path.

The staging app still does not solve live-copy delta replay. It is the
stop-target half of the current transition: the old daemon can remain
canonical while the new target database is staged, but any writes accepted by
the old daemon after staging require another migration stage or a later
high-water-mark replay mechanism.

## Possible features

*Items here are under consideration, not committed. Each names the
open question; moves to the cemented body when settled; retires when
ruled out.*

- **`sema-upgrade-daemon` as upgrade-orchestration daemon.** Today this
  crate is library-only plus Nix-app sandboxes. The persona engine is
  the planned upgrade orchestrator (owner of the active-version
  selector and the start-handover order); whether `sema-upgrade` grows
  its own daemon binary or whether orchestration logic moves into the
  persona engine entirely is open. Lean: a `sema-upgrade-daemon`
  emerges only if the orchestration responsibility separates from
  per-engine concerns.
- **Mirror payload application on the production private upgrade
  socket.** The sandbox path proves migration, selector flip, and
  cross-daemon write isolation, but it does not yet replay mirrored
  write payloads through real daemon sockets. Production cutover
  requires the next daemon to apply Mirror payloads through its
  reverse projection and record `NotRepresentable` results as
  `Divergence`. Open question: where the divergence sink lives in the
  prototype before persona-introspect ships.
- **Retiring `sema-upgrade-handover-temporary`.** The temporary
  external protocol runner exists because the deployed `v0.1.0`
  daemon does not yet own a private upgrade socket. Once `v0.1.0` is
  retrofitted (maintenance build) or a protocol-aware `v0.1.0`
  redeploys, the sandbox shifts to real daemon-to-daemon socket
  exchanges and the temporary runner retires.
