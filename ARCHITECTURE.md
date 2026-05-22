# sema-upgrade Architecture

`sema-upgrade` is the runtime side of the Sema schema-upgrade component.
It owns migration modules, the module index compiled into a given build,
and the execution path for ordinary upgrade attempts.

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

The module carries two shapes:

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
