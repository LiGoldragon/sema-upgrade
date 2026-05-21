# sema-upgrade

`sema-upgrade` is the runtime prototype for component database schema
upgrades.

The first slice is deliberately small: a compile-time migration module
index advertises and runs the `persona-spirit` `0.1.0` to `0.1.1`
upgrade. The execution path uses `signal-executor` with component-local
commands and projects effects into `signal-sema` observation classes.
