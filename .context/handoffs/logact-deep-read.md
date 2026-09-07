# Brief: logact-deep-read

You are the worker **logact-deep-read**, spawned by the controller of this repo
(see AGENTS.md, "Spawned worker"). Runtime: Claude via `claudewho-radiator`.

## Task

Read the full paper **LogAct: Enabling Agentic Reliability via Shared Logs**
(Balakrishnan et al., Meta) at https://arxiv.org/html/2604.07988v1 and write
`research/logact-2604.07988.md`: what it proposes, and what this repo should
borrow, reject, or verify.

This repo builds a Rust CLI + TUI for an append-only JSONL coordination log
(`.context/events.jsonl`) that a controller writes, workers read, and reactors
(a commit reactor, a doc worker) act on with `ack` events. Read
`.context/research/README.md`, `.context/EVENTLOG.md`, and
`skill/event-log-coordination/references/log-reactors.md` first so your
mapping is concrete.

The report must cover, with section references into the paper:
1. The LogAct abstraction (drivers, voters, deciders, executors; AgentBus;
   typed entries and access control) in your own words.
2. Safety guarantees (3.1) and failure/recovery (3.2): what is guaranteed, by
   what mechanism, and under which assumptions.
3. A mapping table: LogAct concept -> this repo's concept (controller, worker,
   reactor, ack, claim, guard, protect-log) -> gap or match.
4. Concrete recommendations for the CLI, ranked, each tagged borrow / reject /
   verify, with the paper's evidence (numbers from the evaluation) or its
   stated limitation as the reason.
5. "Intention before execution": whether a pre-commit `intent` event plus a
   voter/decider step fits a commit reactor, and what it would cost.
6. Related work the paper cites that we should read next (title, venue, why).

Keep it under about 250 lines. Plain English, short sentences. Cite by
section number; never invent a number that is not in the paper.

## Paths you own

- `research/logact-2604.07988.md` (create). Nothing else.

## Log

You MAY append to `.context/events.jsonl`, but only through `append-event.sh`
and only with `by=logact-deep-read` on every line (decision
`log-writers`, see `.context/DECISIONS.md`):

    append-event.sh progress by=logact-deep-read msg="<one line>" ref=research/logact-2604.07988.md
    append-event.sh result   by=logact-deep-read ref=research/logact-2604.07988.md paths=research/logact-2604.07988.md summary="<one line>"

Append one `progress` when you finish reading the paper and one `result` when
the report is done. Never edit, truncate or `rm` the log. Never `git commit`.
Do not start subagents. Do not edit anything outside your path. If you hit
something outside this brief, append `escalate by=logact-deep-read msg=...`
and stop. End your final message with the word DONE.
