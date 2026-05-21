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
- Unsupported component/version pairs return `UpgradeRejected` through
  the contract reply, not a frame-level rejection.

## Prototype Module Index

The first module is:

```text
src/migrations/persona_spirit/version_0_1_0_to_0_1_1.rs
```

The Rust module name is identifier-safe; the public contract still names
the version range as `(0 1 0)` to `(0 1 1)`.
