# Why the model layer is frozen as a contract

`src/model` shipped before any `eventlog` command works, the same way the CLI
surface shipped before any command was implemented (why the CLI surface
shipped first). Later tasks — `log`, `query`,
`react`, `guard`, `scaffold`, `tui` — all read and write `Event`, `Config`,
`Vocabulary`, and `Allowlist`. If two of those tasks could each change a
model signature to suit itself, one could add a field to `Event` that another
task's serializer did not expect, or change what `Allowlist::permits` returns
for an untested case. Freezing the model first, as its own task with its own
tests, means every later task builds against one definition instead of
guessing at each other's.

The reference page is the resulting
contract. This page explains the one piece of it that is easy to get wrong:
how the write allowlist is built up from three sources.

## The three layers, and why the order matters

An allowlist decides which agent may write which event type. It comes from
three places, applied in this order:

1. **The built-in default** (`Allowlist::builtin`): only `controller` writes
   `spawn`, `prompt`, `claim`, `decision`, `retire`, and `approval`; any
   reactor writes the rest.
2. **The config file** (`Allowlist::merge_file`, from `.context/eventlog.toml`):
   replaces the writer list for exactly the types the file names.
3. **`decision key=log-writers` lines in the log** (`Allowlist::apply_decision`),
   applied in `seq` order: each one replaces the *entire* map, not just the
   types it mentions.

The config file layers on top of the defaults; a `decision` in the log
replaces everything before it. That is deliberate, not an inconsistency. The
config file is read once, at startup, so a partial update makes sense: it
only overrides the types the file lists. The log is a history an agent
replays event by event, and each `decision` is a snapshot of what the whole
table should be at that point in time — a partial update there would leave
stale entries around, silently reviving a writer a later line meant to drop.
`controller` never loses `spawn`, `prompt`, `claim`, `decision`, `retire`, or
`approval`, no matter what a decision's value says, so nothing can lock the
controller out of coordinating.

## Why the evaluation order matters

The allowlist is not one fixed table for the whole log: it is evaluated **as
of each line's own `seq`**, by replaying `decision key=log-writers` lines in
order up to that point. This is why revoking a writer never turns history
into a breach. If `survey-x` could write `result` under decision A, and a
later decision B removes that permission, a viewer that checked every past
line against decision B alone would flag `survey-x`'s earlier, legitimate
`result` lines as violations. Replaying decisions in `seq` order and checking
each line against the allowlist that was live when it was written avoids
that: history stays valid, only new writes are constrained by the new rule.
