# Survey: append-only event logs and what this repo should adopt

Prior art for the Rust CLI + TUI that manages `.context/events.jsonl`: an
append-only JSONL coordination log a controller writes, workers read, and
reactors act on with `ack` events. Sources are linked for every claim.

## 1. Event sourcing and CQRS

**Event immutability.** Event sourcing stores every state change as an
append-only sequence of events; the log is the source of truth and current
state is derived by replay.
([Fowler, Event Sourcing](https://www.martinfowler.com/eaaDev/EventSourcing.html))

**Replay to state at N.** Walk events 1..N and you know what the system knew
at N. Fowler compares this to version control: commits are the log, the working
tree is derived state.
([Fowler, Event-Driven](https://www.martinfowler.com/articles/201701-event-driven.html))

**CQRS split.** Command side appends events; query side builds read models from
them. Our log is write-heavy coordination; the TUI is the read model.
([Microsoft, Event Sourcing pattern](https://learn.microsoft.com/en-us/azure/architecture/patterns/event-sourcing))

**Snapshots.** Long replays get slow; periodic snapshots store derived state so
replay starts from a checkpoint, not seq 1.
([Fowler, Event Sourcing — snapshots](https://www.martinfowler.com/eaaDev/EventSourcing.html))

**Upcasting / versioning.** Event schemas evolve; old events are transformed on
read ("upcast") rather than rewritten in the log.
([Young, Versioning in Event Sourcing](https://cqrs.files.wordpress.com/2010/11/versioning-in-eventsourcing.pdf))

**Domain events vs integration events.** Vernon distinguishes in-domain facts
from cross-boundary messages. Our `type` field is closer to integration events:
small facts that coordinate agents, not rich domain payloads.
([Vernon, Implementing DDD — ch. 8](https://vaughnvernon.com/implementing-domain-driven-design/))

**Kleppmann on ordering.** A total order on events simplifies reasoning; our
monotonic `seq` gives that without distributed consensus.
([Kleppmann, Designing Data-Intensive Applications — ch. 9](https://dataintensive.net/))

## 2. The log as the abstraction

**The log defined.** Kreps: a log is an append-only, totally ordered sequence
of records. Databases, replication, Kafka, and git all reduce to this shape.
([Kreps, The Log](https://engineering.linkedin.com/distributed-systems/log-what-every-software-engineer-should-know-about-real-time-datas-unifying))

**Kafka semantics.** Producers append; consumers track an offset and read
forward. Ordering is per-partition; durability comes from replicated append
with fsync before ack.
([Kafka docs, design — log](https://kafka.apache.org/documentation/#design))

**WAL durability.** A write-ahead log must survive process crash: append, then
`fsync` (or `fdatasync`) before reporting success. Without fsync, a crash can
lose the last writes.
([PostgreSQL WAL reliability](https://www.postgresql.org/docs/current/wal-reliability.html))

**Torn writes and line atomicity.** POSIX does not guarantee atomic multi-byte
writes after a crash; a single JSON line can be truncated. Mitigations: write to
a temp file and rename (atomic on same filesystem), or rely on newline-delimited
records small enough that kernel/page behavior is effectively safe — our 4 KB
cap helps. For stronger guarantees, fsync after each append.
([LWN, atomic rename and durability](https://lwn.net/Articles/411842/))

**Our shape.** One JSON object per line, `seq` assigned under a lock, append
via `>>`. Matches Kreps's "simplest storage abstraction" at local scale.

## 3. Exactly-once and idempotent consumers

**The impossibility.** In distributed systems, exactly-once delivery is
simulated by idempotent handlers plus at-least-once delivery plus dedup.
([Kleppmann, ch. 11 — stream processing](https://dataintensive.net/))

**Kafka offsets vs our `ack seq_done`.** Kafka consumers commit offsets; on
restart they skip processed records. Our reactors resume from their own `ack`
events (`seq_done=<seq>`), not a side cursor file — same idea, log-native.
([log-reactors.md](../skill/event-log-coordination/references/log-reactors.md);
[Kafka consumer offsets](https://kafka.apache.org/documentation/#consumerconfigs))

**At-least-once with dedup.** Reactors may see the same event twice if they
crash between acting and acking. Idempotent actions (commit only named `paths=`,
skip if nothing staged) plus ack dedup make this safe.
([log-reactors.md](../skill/event-log-coordination/references/log-reactors.md))

**Outbox pattern.** Microservices write events to an outbox table in the same
DB transaction as business data, then a relay publishes them. Prevents
"dual-write" races. Overkill for a single JSONL file on disk, but the principle
— record intent before side effect — is what `ack` does after the effect.
([Microservices.io, Transactional Outbox](https://microservices.io/patterns/data/transactional-outbox.html))

## 4. Tamper evidence

**Hash chains.** Each entry includes `prev = sha256(previous line)`. Verify
walks the chain; any edit breaks it. Lightweight detection, not prevention.
([Certificate Transparency, RFC 6962 — Merkle audit paths](https://datatracker.ietf.org/doc/html/rfc6962))

**Merkle trees and CT.** Certificate Transparency logs use Merkle trees so
clients get inclusion and consistency proofs. Trillian generalizes this for
append-only transparent logs at scale.
([Trillian docs](https://google.github.io/trillian/))

**Git object model.** Git content is addressed by SHA-1/SHA-256 of `(type,
length, content)`; commits point to trees; history is an append-only chain of
commit objects. Tampering changes hashes downstream.
([Git book — object model](https://git-scm.com/book/en/v2/Git-Internals-Git-Objects))

**OS append-only flags.** macOS `chflags uappnd` and Linux `chattr +a` let
`>>` succeed but block truncate/overwrite/rm. Works across agents (Cursor,
Codex) where IDE hooks cannot. Does not detect edits if someone removes the
flag first.
([protect-log.sh](../skill/event-log-coordination/scripts/protect-log.sh))

**Prevention vs detection.** IDE PreToolUse hooks block rewrites before they
run (Claude Code only). OS flags block at kernel level. Hash chains detect
after the fact. Complementary layers.
([SKILL.md](../skill/event-log-coordination/SKILL.md))

## 5. Concurrency

**Single-writer design.** One writer eliminates ordering ambiguity. Our
controller is the sole writer; workers report back through the controller or
sanctioned `by=` exceptions.
([SKILL.md](../skill/event-log-coordination/SKILL.md))

**mkdir lock vs flock.** `append-event.sh` uses `mkdir` on `<log>.lock/` because
macOS lacks reliable `flock` on all filesystems. Reactors use
`<log>.<name>.reactor.lock/` the same way. Atomic mkdir is the portable choice.
([append-event.sh](../skill/event-log-coordination/scripts/append-event.sh))

**Multi-writer ordering (if ever needed).** Lamport clocks give causal ordering
without a central sequencer; hybrid logical clocks (HLC) add physical time for
debuggability. Only relevant if we relax single-writer — e.g. a remote hub
writer.
([Lamport, Time, Clocks, and Ordering](https://lamport.azurewebsites.net/pubs/time-clocks.pdf);
[Kulkarni et al., HLC](https://www.cse.utexas.edu/~hyunjun/papers/hybrid-time-sands-2014.pdf))

**Seq under lock.** Monotonic `seq` assigned inside the append lock gives a
total order without clocks. Simpler than HLC for local single-writer.

## 6. Retention and size

**Small events + `ref=`.** The log holds coordination facts, not payloads. Large
artifacts live in handoffs; events point by path. Keeps replay and TUI reads
cheap even at hundreds of events.
([EVENTLOG.md](../.context/EVENTLOG.md))

**Compaction.** Kafka log compaction retains the latest record per key and
drops superseded ones. Our log has no key-based supersession — every event is
historically meaningful (audit trail). Compaction would destroy replay fidelity.

**Archiving.** When a log grows large, snapshot derived state (e.g. "agents
active, claims held, last ack per reactor") and archive old segments to
read-only files. Replay from snapshot + tail. Coordination logs in practice
stay small (Drove sample: 283 events); archive is a later concern.

**Size caps.** Rejecting fields > 2 KB and events > 4 KB prevents accidental
transcript injection and keeps lines within safe single-write sizes.
([append-event.sh](../skill/event-log-coordination/scripts/append-event.sh))

---

## Ranked recommendations for the CLI

| # | Recommendation | Source | Verdict | Reason |
|---|----------------|--------|---------|--------|
| 1 | Monotonic `seq` under portable mkdir lock | append-event.sh; Kreps | **Adopt** | Already proven; CLI must preserve lock + seq semantics |
| 2 | Resume reactors from own `ack seq_done`, no side cursors | log-reactors.md; Kafka offsets | **Adopt** | Prevents duplicate commits; cold start is safe replay |
| 3 | Single controller writer; `by=` only for sanctioned reactors/workers | SKILL.md; Fowler immutability | **Adopt** | Core coordination model; CLI enforces vocabulary + `by=` rules |
| 4 | Small events, `ref=` pointers, 2 KB/4 KB caps | EVENTLOG.md; Kleppmann ch. 9 | **Adopt** | Keeps log cheap for controller re-read and TUI render |
| 5 | Idempotent reactor actions + stage only `paths=` from decision | log-reactors.md | **Adopt** | At-least-once safety without exactly-once infrastructure |
| 6 | One reactor instance via mkdir lock holding pid | log-reactors.md | **Adopt** | Stops duplicate instances racing the same event |
| 7 | Opt-in OS append-only (`chflags`/`chattr`) per log | protect-log.sh; SKILL.md | **Adopt** | Cross-agent enforcement where IDE hooks do not run |
| 8 | `fsync` after append (configurable) | PostgreSQL WAL; LWN durability | **Adopt** | Optional strict mode for crash-safe single-line durability |
| 9 | Hash chain `prev` field + verify subcommand | RFC 6962; CT model | **Defer** | Useful audit proof; add when tamper-evidence is a stated requirement |
| 10 | PreToolUse append-only guard | eventlog-guard.sh | **Defer** | Claude Code only; document as optional install, not CLI core |
| 11 | Periodic snapshots of derived coordination state | Fowler snapshots | **Defer** | Logs stay small; replay-from-1 is fine until profiling says otherwise |
| 12 | Event schema upcasting / version field | Young versioning PDF | **Defer** | Flat string fields evolve cheaply; formal upcasting when schema breaks |
| 13 | Merkle tree / Trillian-style transparency proofs | Trillian; RFC 6962 | **Reject** | Local agent coordination does not need inclusion proofs |
| 14 | Log compaction by key | Kafka compaction | **Reject** | Every coordination event is part of the audit trail |
| 15 | Multi-writer with HLC / Lamport clocks | Lamport; HLC paper | **Reject** | Single-writer is intentional; hub-writer is a separate design |
| 16 | Transactional outbox to separate publish step | microservices.io outbox | **Reject** | Single-file append already atomic enough at this scale |
| 17 | Segment archiving + snapshot replay | Kreps log; Fowler snapshots | **Defer** | Needed only when logs exceed comfortable full replay (10k+ events) |
| 18 | `flock` instead of mkdir lock | — | **Reject** | macOS portability already drove mkdir choice |
| 19 | Reactor runs in dedicated terminal, not agent shell | log-reactors.md | **Adopt** | CLI `reactor run` should assume foreground supervisor (TUI pane) |
| 20 | Baseline ack at first reactor start (tip seq) | log-reactors.md | **Adopt** | Prevents replaying historical decisions on fresh install |

**Summary.** Adopt what the skill already does well: append-only JSONL, locked
seq, small `ref=` events, log-native ack cursors, idempotent reactors, and
opt-in OS protection. Defer snapshots, hash chains, and archiving until scale or
compliance demands them. Reject distributed-log machinery (compaction, Merkle
proofs, multi-writer clocks, outbox) — this repo is a local coordination spine,
not a Kafka cluster.
