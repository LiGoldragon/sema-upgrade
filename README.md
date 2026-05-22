# sema-upgrade

`sema-upgrade` is the runtime prototype for component database schema
upgrades.

The first slice is deliberately small: a compile-time migration module
index advertises and runs the `persona-spirit` `0.1.0` to `0.1.1`
upgrade. The execution path uses `signal-executor` with component-local
commands and projects effects into `signal-sema` observation classes.

The temporary `sema-upgrade-temporary` binary exists only to exercise the
first database migration before the daemon is complete. It takes one NOTA
argument or one path to a NOTA file:

```text
(Attempt (/tmp/persona-spirit-v0.1.0.redb /tmp/persona-spirit-v0.1.1.redb (persona-spirit (0 1 0) (0 1 1))))
```

The binary refuses missing source paths, existing target paths, and
unsupported component/version pairs. The current migration reads the
historical Spirit `Certainty` record shape and writes the current
`Magnitude` record shape into a new database.

`src/handover.rs` contains the first private-upgrade socket handover
prototype. It uses `signal-version-handover` records for marker/readiness/
completion/mirror/divergence messages and `version-projection` for per-type
projection. The prototype is tested, but it is not yet wired into the
production `persona-spirit` daemon sockets.

For the live sandbox path, use the Nix app so the daemon, CLI, and
migration tool are all selected by the flake:

```sh
nix run --option max-jobs 0 .#spirit-migration-sandbox -- /home/li/.local/state/persona-spirit/persona-spirit.redb
```

The app copies the source database to a temporary directory, migrates the
copy, starts the tagged `persona-spirit` `v0.1.1` daemon on temporary
sockets, queries it through the tagged `spirit` CLI, writes a sandbox-only
`High` record, and verifies that record can be queried back.

For the two-version smart-handover sandbox, use:

```sh
nix run --option max-jobs 0 .#spirit-smart-handover-sandbox -- /home/li/.local/state/persona-spirit/v0.1.0/persona-spirit.redb
```

The app copies the latest `v0.1.0` database into an isolated sandbox, starts
the tagged `v0.1.0` daemon, writes a legacy-compatible record there, snapshots
that database, migrates the snapshot to `v0.1.1`, starts the tagged `v0.1.1`
daemon, runs the current `signal-version-handover` protocol prototype, flips
the sandbox's public selector to the next daemon, writes a `High` record
through `v0.1.1`, and verifies that the old database did not receive the
next-only write.

For the persistent staging path, stop the target `v0.1.1` daemon first,
then run:

```sh
nix run --option max-jobs 0 .#spirit-migration-stage -- \
  /home/li/.local/state/persona-spirit/v0.1.0/persona-spirit.redb \
  /home/li/.local/state/persona-spirit/v0.1.1/persona-spirit.redb
```

The staging app migrates into a same-directory temporary database, copies that
database into a probe database, starts a tagged `persona-spirit` `v0.1.1`
daemon against the probe, verifies queries and a `High` record there, backs up
any existing target database, then atomically moves the unmodified staged
database into the requested target path.
