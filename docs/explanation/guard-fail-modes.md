# Why the guard fails open on non-JSON, closed on an unknown shape, checks command shape instead of a substring, and ignores operators inside quotes

[`eventlog guard`](../reference/guard.md) sits in the hot path of every tool
call an agent makes. Two design choices keep it safe without making it
block work it was never meant to judge.

## Fail open on non-JSON, fail closed on an unknown shape

`parse` returns two different errors, and `eventlog guard` treats them
oppositely:

- Input that is not JSON at all (`ParseFail::NotJson`) allows the call.
- JSON in a shape `parse` does not recognize (`ParseFail::UnknownShape`)
  denies it.

The guard cannot assume every hook invocation carries a payload it
understands. Claude, Cursor, and Codex each send their own JSON shape, and
future agents or tool versions will send others. If a payload is not even
JSON, the safest reading is that something other than a tool-call hook put
it there — a stray environment probe, a manual test run — and guessing
wrong would block every tool call in the session on a bug in the guard
itself, not a real policy violation.

JSON in an unrecognized shape is a different case. The guard was called as
a hook, so *something* is asking it to judge a real tool call; it just
cannot tell what the call does. A payload it cannot read is a payload it
cannot clear, so it denies instead of guessing "probably safe." The two
failure modes trade off differently: failing open on `NotJson` risks
letting through input the guard was never asked to police, and failing
open on `UnknownShape` risks letting a genuine mutation through
unread. Only the second risk touches the log directly, so only the second
case fails closed.

## Why the sanctioned-writer check parses shape, not a substring

`is_simple_sanctioned_writer` does not check whether a command *contains*
`eventlog` or `append-event.sh`. It requires the command to be one simple
command — no `;`, `&&`, `||`, `|`, `$(`, backtick, newline, redirect, or
`&` — whose first word is exactly that binary.

A substring check is easy to defeat by construction, not just by
accident: `append-event.sh result ...; rm .context/events.jsonl` contains
the sanctioned binary's name and still deletes the log. Requiring the
binary to be argv[0] of a command with no operators closes that gap,
because a compound command by definition has more than one command in it,
and the sanctioned-writer exemption only ever covers exactly one.

## Why an operator inside quotes is not compound

`decide` used to scan a `Shell` command's full text for `;`, `&&`, `||`,
`|`, `$(`, a backtick, or a newline, and denied on a match. That broke the
one read the coordination workflow recommends for filtering the log: a
quoted `jq` filter such as `jq -c 'select(.type=="ack") | {seq}'
.context/events.jsonl`. The `|` sits inside the single-quoted filter
argument, where the shell treats it as a literal character, not a pipe.
The guard denied the command anyway, because it read the text, not what
the shell would run.

`decide` now scans only `outside_quotes(command)`, which strips quoted
text before the operator check. A single-quoted span hides everything
between the quotes, since the shell never expands anything inside single
quotes. A double-quoted span keeps `` ` `` and `$(`, because the shell
still expands a backtick or a command substitution inside double quotes,
so a command hidden that way must still be caught. This makes the check
match what the shell itself would treat as one command instead of
flagging any command that merely mentions an operator character.
