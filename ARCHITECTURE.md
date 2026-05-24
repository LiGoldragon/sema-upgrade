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

## Pending schema-engine upgrade

**Status:** scheduled for migration to schema-language-based contract per `reports/designer/326-v13-spirit-complete-schema-vision.md` + `reports/designer/324-migration-mvp-spirit-handover-re-specification.md`.

**Target:** the substantive migration surface has already moved to the `upgrade` triad; what little remains in this transitional library either folds into the upgrade triad's schema cutover or is retired before its own cutover lands. There is no separate `sema-upgrade/sema-upgrade.schema` target on the roadmap.

**Sequence:** Spirit pilots `primary-ezqx.1` first; the upgrade triad's schema cutover follows. This library's cutover **may coincide with the library retiring entirely** as the upgrade triad takes over its role — preferred outcome is deletion of this repo when the last old pin is gone, not a separate schema cutover.

**Per-component concerns:**
- Transitional / breadcrumb crate per /318 retirement and /317-1. `PrototypeHandover` retired per /317-1; the load-bearing migration runtime now lives in `upgrade` and the working contract in `signal-upgrade`.
- This crate must not grow new migration modules; if a schema-cutover bead were ever opened for `sema-upgrade`, treat it as a sign that work landed in the wrong place — redirect to `upgrade` / `signal-upgrade`.
- Likely outcome: this section becomes moot when the crate is deleted.

**References:**
- `reports/designer/326-v13-spirit-complete-schema-vision.md` — uniform header form + schema-language design
- `reports/designer/324-migration-mvp-spirit-handover-re-specification.md` — migration MVP + handover state
- `reports/designer/322-spirit-mvp-positional-schema-worked-example.md` — Spirit MVP worked example
- `reports/operator/174-schema-import-header-design-critique-2026-05-24.md` — header/body/feature separation + lowering rules
