# Why the Drovefile pins the herdr session

`drove up` needs a herdr session to place its panes in. Without a pinned
session, `drove` had to infer one — usually from the shell's current
session — so running `drove up` from a different shell than the one that
started the layout could resolve the wrong session, or fail to resolve one
at all.

The `Drovefile` now declares the session up front:

```
herdr.session("event-log")
```

`drove up` uses this name from any shell, so starting or re-attaching to the
layout no longer depends on which shell you happen to run it from. An
explicit `--session` flag or a `HERDR_SESSION` environment variable still
overrides the pinned name — the Drovefile sets the default, not a fixed
value.

## See also

- [Why the controller pane no longer spawns a second Claude agent](controller-caller-pane.md) — another Drovefile change that makes `drove up` behave the same regardless of where you run it from.
