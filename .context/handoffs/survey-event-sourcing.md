# Brief: survey-event-sourcing

You are the worker **survey-event-sourcing**, spawned by the controller of this
repo (see AGENTS.md, "Spawned worker"). Runtime: cursor-agent, auto model.

## Task

Survey the engineering prior art for append-only event logs and write
`research/survey-event-sourcing.md`: what the field already knows about doing
this efficiently and safely, and which of it this repo should adopt.

This repo builds a Rust CLI + TUI for an append-only JSONL coordination log
(`.context/events.jsonl`) that a controller writes, workers read, and reactors
act on and acknowledge with `ack` events. Read `.context/research/README.md`,
`.context/EVENTLOG.md`, `skill/event-log-coordination/SKILL.md` and
`skill/event-log-coordination/references/log-reactors.md` first.

Cover, with a source (paper, book, project docs or post; URL) for every claim:
1. Event sourcing and CQRS: event immutability, upcasting/versioning, snapshots,
   replay to state at N. Sources: Fowler, Young, Vernon, Kleppmann.
2. The log as the abstraction: Kreps "The Log", Kafka semantics, WAL and
   durability (fsync, torn writes, single-line atomicity of appends).
3. Exactly-once and idempotent consumers: offsets vs our `ack seq_done`
   pattern, at-least-once with dedup, outbox pattern.
4. Tamper evidence: hash chains, Merkle trees, Certificate Transparency style
   logs, Trillian, `git` object model; append-only file flags (chflags/chattr)
   and their limits.
5. Concurrency: single-writer designs, mkdir locks vs flock, multi-writer
   ordering (Lamport clocks, hybrid logical clocks) if we ever allow
   multi-writer.
6. Retention and size: compaction, archiving, and why small events + `ref=`
   pointers work.

End with a ranked recommendations table for the CLI: recommendation, source,
adopt / defer / reject, one-line reason. Under about 250 lines. Plain
English, short sentences.

## Paths you own

- `research/survey-event-sourcing.md` (create). Nothing else.

## Log

You MAY append to `.context/events.jsonl`, but only through `append-event.sh`
(on PATH) and only with `by=survey-event-sourcing` on every line (decision
`log-writers`, see `.context/DECISIONS.md`):

    append-event.sh progress by=survey-event-sourcing msg="<one line>" ref=research/survey-event-sourcing.md
    append-event.sh result   by=survey-event-sourcing ref=research/survey-event-sourcing.md paths=research/survey-event-sourcing.md summary="<one line>"

Append one `progress` at the halfway point and one `result` when the report
is done. Never edit, truncate or `rm` the log. Never `git commit`. Do not
start subagents. Do not edit anything outside your path. If you hit something
outside this brief, append `escalate by=survey-event-sourcing msg=...` and
stop. End your final message with the word DONE.
