# Why the controller pane no longer spawns a second Claude agent

The `control` workspace's `coordinator` tab used to declare its controller
pane like this:

```
pane("controller", agent = agent(
    kind = "claude",
    prompt = "You are the controller for this repo. Read .context/handoffs/bootstrap.md and follow it.",
))
```

`drove up` would start this pane and launch a brand-new Claude agent inside
it. But the person who runs `drove up` almost always types it from a Claude
pane they're already in. Spawning a second agent for the `controller` role
left two Claude processes doing one job: the one the user was already
talking to, and the new one holding the `controller` role.

With drove 0.1.0's v3 API, a pane can claim the process that invoked `drove
up` instead of starting a new one:

```
caller_pane("controller")
```

The `Drovefile` now uses this for the `control › coordinator` pane. The pane
that runs `drove up` *becomes* the controller; no second agent is started.
This matches how `drove up` already reconciles an existing layout on a
repeat run (eventlog seq 402), re-attaching to panes instead of duplicating
them. `caller_pane` extends that same behavior to the first run.

Agents that read [AGENTS.md](../../AGENTS.md) still identify the controller
the same way — "the Claude in the `control › coordinator` pane" — regardless
of whether that pane was reached via `caller_pane` or a spawned agent. Only
how the pane starts changed, not who counts as the controller.

## See also

- [Why the reactor panes hand their loop to `eventlog react`](reactor-runtime-switch.md) — the equivalent story for the `maintenance` workspace's reactor panes, which are still spawned agents, not `caller_pane`.
