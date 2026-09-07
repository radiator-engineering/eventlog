# Brief: survey-agent-coordination

You are the worker **survey-agent-coordination**, spawned by the controller
of this repo (see AGENTS.md, "Spawned worker"). Runtime: cursor-agent, auto
model.

## Task

Survey the research and practice on coordinating multiple LLM coding agents
safely and write `research/survey-agent-coordination.md`.

This repo builds a Rust CLI + TUI for an append-only JSONL coordination log
(`.context/events.jsonl`) that a controller writes, workers read, and reactors
act on. Its vocabulary: spawn, prompt, claim, progress, result, decision,
escalate, approval, retire, ack, violation, seam. Read
`.context/research/README.md`, `.context/EVENTLOG.md` and
`skill/event-log-coordination/SKILL.md` first.

Cover, with a source (URL; prefer arXiv, then vendor docs) for every claim:
1. Shared-log and blackboard architectures for agents: LogAct
   (arXiv 2604.07988) only briefly (another worker reads it in depth), plus
   its related work; blackboard systems; event-driven agent frameworks.
2. Safety before execution: intent logging, voters/verifiers, approval gates,
   human-in-the-loop patterns; AgentDojo and similar injection benchmarks.
3. Coordination protocols: MCP, A2A, AG-UI; what they say about ordering,
   idempotency and audit.
4. Parallel coding agents: file ownership / claims, worktrees, merge seams,
   conflict detection; what SWE-agent style systems, Devin-like products, and
   Cursor/Claude Code multi-agent docs actually do.
5. Replay, audit and debugging of agent runs: trace formats (OpenTelemetry
   GenAI conventions, LangSmith, AgentOps), and what an audit log must hold
   to answer "who knew what when".
6. Failure modes reported in the literature: duplicate actions, lost acks,
   stale state, context bleed, runaway loops; and the mitigations.

End with a ranked recommendations table for the CLI: recommendation, source,
adopt / defer / reject, one-line reason. Under about 250 lines. Plain
English, short sentences.

## Paths you own

- `research/survey-agent-coordination.md` (create). Nothing else.

## Log

You MAY append to `.context/events.jsonl`, but only through `append-event.sh`
(on PATH) and only with `by=survey-agent-coordination` on every line
(decision `log-writers`, see `.context/DECISIONS.md`):

    append-event.sh progress by=survey-agent-coordination msg="<one line>" ref=research/survey-agent-coordination.md
    append-event.sh result   by=survey-agent-coordination ref=research/survey-agent-coordination.md paths=research/survey-agent-coordination.md summary="<one line>"

Append one `progress` at the halfway point and one `result` when the report
is done. Never edit, truncate or `rm` the log. Never `git commit`. Do not
start subagents. Do not edit anything outside your path. If you hit something
outside this brief, append `escalate by=survey-agent-coordination msg=...`
and stop. End your final message with the word DONE.
