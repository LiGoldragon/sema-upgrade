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
