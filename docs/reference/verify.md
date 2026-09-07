# Verify: `src/log/verify.rs` and `eventlog verify`

Status: implemented. Walks the hash chain described in section 3 of [the
design spec](../superpowers/specs/2026-09-06-event-log-cli-design.md) and
reports the first break.

```rust
pub struct VerifyReport {
    pub checked: u64,
    pub last_good: u64,
    pub failure: Option<String>,
}

pub fn verify(log: &Log) -> anyhow::Result<VerifyReport>;
```

`verify` reads the whole file with `Log::open` and parses
each line with `Event::parse_line`. It stops at the first problem and
returns a report rather than an error; `anyhow::Result` only wraps I/O
failures reading the file.

| Field | Meaning |
|---|---|
| `checked` | Number of lines verified before the failure (or the whole file, if `failure` is `None`). |
| `last_good` | `seq` of the last line that checked out. |
| `failure` | `None` if the chain is intact; otherwise one of the messages below. |

## What counts as a failure

Lines before the first `prev` are a pre-chain prefix and always pass — an
older log can start without a hash chain and gain one partway through. Once
a line carries `prev`, every later line must too:

| Failure | Cause |
|---|---|
| `malformed line N: <reason>` | The line at line number `N` does not parse as an `Event`. |
| `chain break at seq N` | Line `N` carries `prev`, but it does not equal `Log::hash_line` of the previous line's raw bytes (or `"genesis"`, if `N` is line 1). |
| `unchained line at seq N` | Line `N` has no `prev`, but an earlier line already did — the chain cannot un-start. |

See [why the chain can start partway through a log](../explanation/hash-chain-verification.md).

## `eventlog verify`

```sh
eventlog verify [--log <name|path>]
```

Prints `ok: N events, chain intact` and exits 0 on success. On failure,
prints the failure message and the last good `seq` to stderr and exits 1:

```
chain break at seq 4 (last good seq: 3)
```

## Fixtures and tests

`tests/fixtures/chain_ok.jsonl`, `chain_break.jsonl` and
`chain_unchained_after.jsonl` are small, hand-built four-line logs, one per
case above. `tests/log_verify.rs` checks each fixture and one mutation case:
stripping `prev` from every line but the first in `chain_ok.jsonl` must fail
with an `unchained` message. Run them with:

```sh
cargo test
```

## See also

- Log module — `Log::open` and `Log::hash_line`, which `verify` builds on.
- [Why the chain can start partway through a log](../explanation/hash-chain-verification.md)
- [`eventlog` command list](eventlog-cli-surface.md)
