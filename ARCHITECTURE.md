# sema-upgrade Architecture

`sema-upgrade` is retired.

The `/318` upgrade-triad merger moved the load-bearing migration
surface into the `upgrade` component triad:

- `upgrade` owns the runtime migration catalogue, command executor,
  and adjacent-version handover driver.
- `signal-upgrade` owns the ordinary working signal contract.
- `owner-signal-upgrade` owns the owner-only policy signal contract.
- `version-projection` remains a library and now names its runtime
  projection lookup as `RuntimeMigrationLookup`.

This repo stays buildable only as a compatibility breadcrumb for old
pins and historical links. It must not grow new migration modules,
temporary runners, handover sockets, or signal contracts. New work
lands in `upgrade` or in the relevant signal contract crate.

## Nix Surface

The flake intentionally keeps a small package and check set so old
references fail loudly if the crate stops compiling, but the crate has
no daemon, no CLI, and no runtime authority.
