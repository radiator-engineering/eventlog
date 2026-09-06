# Why the chain can start partway through a log

[`verify`](../reference/verify.md) lets a log open with lines that carry no
`prev` at all, as long as they all come before the first line that does. Once
one line carries `prev`, every line after it must too.

## The problem this avoids

The hash chain (`prev` links, [log format](../superpowers/specs/2026-09-06-event-log-cli-design.md))
is written and verified from v1 of `eventlog`, but logs written before v1
already exist — this repo's own `.context/events.jsonl` is one. Requiring
every line in every log to carry `prev`, including lines written by the old
shell toolkit, would make every pre-existing log fail verification on day
one. That is not a real tamper: no line was altered, the log just predates
the chain.

## The fix

`verify` treats the run of `prev`-less lines at the start of a file as a
pre-chain prefix, not a defect. The first chained line still has to prove it
follows that prefix: its `prev` must hash the last pre-chain line, exactly as
if the pre-chain lines were one block committed at once. So the chain binds
the whole file from that point, but does not retroactively demand a hash for
history recorded before it existed.

## Why the chain cannot stop once started

The reverse case — a chained line followed by a line with no `prev` — is
never legitimate, so `verify` treats it as `unchained line at seq N`, a
failure, not a second pre-chain region. A tool cannot tell "the chain ended
here on purpose" apart from "a line was deleted and its neighbors no longer
hash-link, so whoever edited the file dropped `prev` rather than recompute
it." Only one direction of transition — no `prev` to `prev` — has an honest
reading. The other direction is treated as tampering by construction, because
allowing it would let anyone silently truncate the verified part of a log by
just omitting one field.
