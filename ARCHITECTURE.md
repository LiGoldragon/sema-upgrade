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
