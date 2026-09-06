# Survey: coordinating multiple LLM coding agents safely

Research for the event-log CLI (2026-09-06). This repo uses an append-only JSONL log (`.context/events.jsonl`) as coordination spine: spawn, prompt, claim, progress, result, decision, escalate, approval, retire, ack, violation, seam. LogAct is covered only briefly; another worker reads it in depth.

---

## 1. Shared-log and blackboard architectures

**LogAct (brief).** Meta's LogAct (arXiv:2604.07988) treats each agent as a deconstructed state machine over a durable append-only **AgentBus**. Intentions appear in the log before execution; pluggable **Voters** approve or reject; a **Decider** commits; an **Executor** materializes side effects. Multi-agent swarms coordinate via mailbox messages on each other's buses; a supervisor can introspect logs to cut redundant work (17% more unique files, 41% fewer tokens in their swarm experiment). Related work cited in the paper includes shared-log systems (Balakrishnan et al. 2012–2021), deterministic replay substrates (Jia & Witchel 2021), and recent agent reliability work (Luo et al. 2024; Bhat et al. 2025).

**Blackboard systems.** Classic blackboard architectures (Hayes-Roth 1985, *Artificial Intelligence* 26(3)) use a shared structured store where independent **knowledge sources** read and write partial solutions under a scheduler. BB1 (Hayes-Roth 1984) adds a separate **control blackboard** so scheduling itself is data the system can inspect, override, and explain. The pattern matches today's coordination logs: facts on a board, a controller decides what runs next, humans can intervene.

**Event-driven agent frameworks.** AutoGen Core (Microsoft docs) is actor-model and event-driven: agents communicate only through asynchronous messages and pub/sub topics. LangGraph (AgentGit paper, arXiv:2511.00628) models workflows as state graphs with checkpointing and deterministic control flow. Production orchestration often blends DAG, event, and actor patterns (Zylos Research 2026). This repo's log is closer to a blackboard + event log than to a full message bus: small typed events, big artifacts referenced by path.

---

## 2. Safety before execution

**Intent-before-execute.** LogAct's core safety property is that intentions are durable and visible before side effects (arXiv:2604.07988). Dual voters cut attack success to 1.4% on their benchmark with ~3% benign utility loss. The architecture mixes rule-based verifiers (hard guarantees on defined properties) with LLM voters (broader, probabilistic).

**Human-in-the-loop and gates.** MCP's tools spec says clients **SHOULD** prompt for user confirmation on sensitive operations and show tool inputs before calling the server (modelcontextprotocol.io, Tools 2026-07-28). That is UI-level gating, not a durable audit record. This repo's `escalate` / `approval` events are closer to explicit workflow gates.

**Benchmarks: AgentDojo.** AgentDojo (arXiv:2406.13352) evaluates agents that call tools over untrusted data. It reports benign utility (BU), utility under attack (UA), and attack success rate (ASR). GPT-4o baseline: 69% BU, 45% UA, up to 53% ASR on some attacks. Defenses (tool filtering, prompt sandwiching, classifiers) trade security for utility. Pipeline defenses plug in as `BasePipelineElement` stages (agentdojo.spylab.ai). For a coding coordination CLI, the lesson is: log the *intent* to run a tool or commit, verify before reactors act.

**Voters and verifiers.** LogAct voters inspect intentions on the bus before commit. AgentDojo's tool filter and TransformersBasedPIDetector show the same pattern at the tool-output boundary. A coordination log can record `decision` and `approval` as first-class votes without running inference inside the log itself.

---

## 3. Coordination protocols: MCP, A2A, AG-UI

**MCP (Model Context Protocol).** MCP standardizes agent-to-tool calls: servers expose tools; clients invoke them over JSON-RPC (modelcontextprotocol.io). The spec does **not** define a cross-session audit log or ordering semantics for multi-agent work. It recommends clients log tool usage and confirm sensitive calls. Persistence, idempotency, and attribution are implementation concerns (stingrai.io analysis of MCP 2025-11-25).

**A2A (Agent2Agent).** Google's A2A v1.0 (a2a-protocol.org) standardizes agent-to-agent task delegation: Agent Cards at `/.well-known/agent.json`, stateful **Task** objects with defined lifecycle states, cursor-based pagination, and proto-first spec (RFC 8785 canonicalization for signed cards). A2A complements MCP (tools) and targets cross-vendor agent collaboration. Ordering is per-task stream semantics, not a global append-only coordination log.

**AG-UI (Agent–User Interaction).** AG-UI (docs.copilotkit.ai) is an event-based protocol between agent backends and user-facing apps: streaming text, tool visualization, shared state, human-in-the-loop. It standardizes the **UI boundary**, not file ownership or git merge policy. Layering: MCP for tools, A2A for agents, AG-UI for users (CopilotKit docs).

**MACP (related).** The Multi-Agent Coordination Protocol (multiagentcoordinationprotocol.io) specifies session-scoped **runtime acceptance order**, append-only session history, `message_id` deduplication, and replay integrity under identical mode versions. That is the closest formal protocol to this repo's `seq` + append-only JSONL, though MACP targets distributed agent sessions rather than a local dev workspace.

**Audit and idempotency gap.** None of MCP, A2A, or AG-UI replace a project-local coordination log. They move bytes between components; this CLI records *who decided what, when* for replay inside a repo.

---

## 4. Parallel coding agents

**SWE-agent (single agent, rich ACI).** SWE-agent (arXiv:2405.15793, NeurIPS 2024) shows that interface design dominates raw model power for coding agents: custom file editor, repo navigation, and context management in an agent-computer interface (ACI). It is primarily **one agent, one repo checkout**, not a multi-agent ownership protocol. Later systems (Open SWE, langtalks/swe-agent) add planner/developer splits and subagents via LangGraph.

**Devin-like products: worktrees for isolation.** Devin Desktop / Cascade run parallel sessions in **git worktrees** so each conversation edits an isolated copy; merge brings changes back (docs.devin.ai/worktrees). `post_setup_worktree` hooks copy `.env` and install deps. Isolation is filesystem-level; overlap is deferred to merge time unless tasks are pre-partitioned.

**Cursor.** Subagents run in separate context windows; foreground or background modes (cursor.com/docs/subagents). Parallel launch sends multiple Task calls in one turn. `/multitask` runs async subagents instead of queuing (cursor.com/help/ai-features/multi-agent). Cursor acknowledges early parallel runs lack strong conflict prevention; coordination is mostly model-driven. Cloud Agents use isolated VMs (cursor.com/docs/sdk/python).

**Claude Code.** Parallel sessions use `--worktree` or agent-view background sessions that auto-move to `.claude/worktrees/` (code.claude.com/docs/en/worktrees). Subagents can set `isolation: worktree`; the product blocks git commands that escape the worktree (code.claude.com/docs/en/subagents). Task subagents inherit parent isolation; nested subagents up to depth 3 by default (changelog).

**Claims, seams, and detection.** This repo's `claim` + `check-claims.sh` + `violation` pattern is explicit ownership in the log—stronger than prose in AGENTS.md alone (event-log-coordination skill). Industry practice: partition by directory before parallelizing (MindStudio, Developers Digest Claude Code tutorial); record cross-worker dependencies as **seams** at discovery time. Worktrees isolate files; claims make ownership replayable; seams name integration debt early.

---

## 5. Replay, audit, and debugging

**OpenTelemetry GenAI.** OTel GenAI semantic conventions (opentelemetry.io/blog/2026/genai-observability) standardize spans for LLM calls and tool execution: model id, token counts, finish reasons; content capture is opt-in. Span trees (`invoke_agent` → `chat` → `execute_tool`) support debugging but are optimized for observability backends, not tamper-evident coordination.

**ATSC and audit signal.** Agent Telemetry Semantic Conventions (github.com/agent-telemetry-spec/atsc) extend OTel for agent reasoning cycles, handoffs, and `human.audit` events. OpenTelemetry is also adding a dedicated **Audit Logging** signal with RecordId idempotency, hash chains, and guaranteed delivery (opentelemetry-specification PR #5059). Coordination logs and OTel traces should share a correlation id (`trace_id` / `thread_id`).

**LangSmith.** LangSmith's `coding-agent-v1` metadata contract requires `ls_agent_type`, `thread_id`, `ls_integration`, and git context fields on coding-agent runs (docs.langchain.com/langsmith/coding-agent-metadata-contract). Cursor and Claude Code plugins emit one trace per turn grouped by `thread_id`. That answers "what did the model do?" more than "what did the controller authorize?"

**AgentOps.** AgentOps (docs.agentops.ai) builds on OTel: sessions, nested spans, handoffs, time-travel replay in a dashboard. Public API exposes trace/span read paths. Good for runtime debugging; weak as a project-local source of truth for merge/commit policy.

**What an audit log must hold for "who knew what when".** Minimum set aligned with literature and this repo:
- Monotonic sequence and UTC timestamp (replay cutoff = event N).
- Actor identity (`agent`, `by`, `from`/`to`).
- Event type and small flat fields; large payloads by `ref=` path only.
- Lifecycle closure: spawn → prompt → result → retire.
- Reactor progress: `ack` with `seq_done` (resume from log, not side files).
- Optional: hash chain (`prev`) for tamper evidence; correlation to OTel trace id.

Walking the log to seq N reconstructs coordination state without any agent's transcript.

---

## 6. Failure modes and mitigations

| Failure | How it shows up | Mitigations (with sources) |
|--------|------------------|----------------------------|
| Duplicate actions | Retry after lost ack; reactor replays `result` | Reactor `ack` + idempotent staging; idempotency keys on mutating tools (tianpan.co 2026; LogAct single in-flight intention) |
| Lost acks | Second commit or double deploy | Reactors resume from own `ack` events only (event-log-coordination skill); MACP `message_id` dedup |
| Stale state | Worker reads pre-update shared memory | Serialize shared foundation first; optimistic versioning on shared records (ecoaai.com; tianpan.co 2026) |
| Context bleed | Pasting transcripts between panes | Single-writer log; workers report to controller; brief forbids log append (AGENTS.md) |
| Runaway loops | Agent retries until budget exhausted | Escalate to human; cap turns; record `progress` for liveness |
| Parallel file clash | Two agents edit same path | `claim` at spawn; worktrees per worker (`decision key=agent-topology`); `violation` on `result` |
| Misdiagnosed "hallucination" | Downstream agent reasons over corrupted shared state | Trace read/write versions; treat as concurrency bug (tianpan.co 2026-04-12) |
| Prompt injection via tools | Agent exfiltrates secrets | AgentDojo-style defenses; voters before execute (LogAct); MCP confirm + log (spec) |

AgentDojo shows even strong models fail security tests without architectural defenses—not just better prompts (arXiv:2406.13352).

---

## 7. Ranked recommendations for the event-log CLI

| Rank | Recommendation | Source | Adopt / defer / reject | Reason |
|------|----------------|--------|------------------------|--------|
| 1 | Append-only JSONL with monotonic `seq`, small events, `ref=` pointers | LogAct AgentBus; MACP session log; this repo EVENTLOG.md | **Adopt** | Core product thesis; enables replay and audit |
| 2 | Single controller writer; workers return results out-of-band | event-log-coordination skill; LogAct separation of Driver/Voter/Executor | **Adopt** | Stops context bleed; already in AGENTS.md |
| 3 | `claim` at spawn + `check-claims.sh` at `result` | Cursor/Claude parallel docs (implicit); explicit in skill | **Adopt** | Makes file ownership machine-checkable, not honor-system |
| 4 | Reactors ack `seq_done`; stage only `paths=` from events | skill reactor-example; LogAct execute-after-commit | **Adopt** | Prevents duplicate commits and scope creep |
| 5 | Record `seam` when workers report integration dependencies | Multi-agent merge practice; blackboard explicit deps | **Adopt** | Cheap structured input to merge planning |
| 6 | `escalate` / `approval` before irreversible reactor actions | MCP HITL SHOULD; LogAct voters | **Adopt** | Human gate for commit/deploy without blocking read-only work |
| 7 | OS-level append-only (`protect-log.sh`) for non–Claude Code agents | skill portability section; LogAct durable log | **Adopt** | Hook alone insufficient under Cursor/Codex |
| 8 | Optional hash chain (`prev`) on log lines | OTel audit PR #5059; skill mentions | **Defer** | Tamper-evidence nice; `chflags uappnd` covers most cases first |
| 9 | Intent/vote/decide/execute event types before tool run | LogAct full protocol | **Defer** | Heavy for v1; `approval` + reactors cover commit path |
| 10 | Built-in LLM voter on every shell command | LogAct; AgentDojo defenses | **Defer** | High latency/cost; rule-based guard + human escalation first |
| 11 | Emit OTel GenAI spans from CLI | opentelemetry.io GenAI; LangSmith contract | **Defer** | Complement log via shared `thread_id`; not replacement |
| 12 | Implement MCP/A2A/AG-UI servers in CLI | a2a-protocol.org; CopilotKit AG-UI | **Reject** | Out of scope; CLI is local coordination spine, not wire protocol |
| 13 | Rely on model self-coordination for parallel edits | Cursor forum on `/multitask` v0 | **Reject** | Documented gap; use claims/worktrees instead |
| 14 | Shared Redis/DB as team memory between agents | AutoGen/CrewAI patterns; concurrency posts | **Reject** | Invites lost-update races; log + git are source of truth |

---

## Sources (quick index)

- LogAct: https://arxiv.org/abs/2604.07988
- Hayes-Roth blackboard: https://doi.org/10.1016/0004-3702(85)90063-3
- AutoGen Core: https://microsoft.github.io/autogen/stable/user-guide/core-user-guide/
- AgentDojo: https://arxiv.org/abs/2406.13352
- MCP Tools spec: https://modelcontextprotocol.io/specification/2026-07-28/server/tools
- A2A v1.0: https://a2a-protocol.org/v1.0.0/
- AG-UI: https://docs.copilotkit.ai/ag-ui/introduction
- MACP determinism: https://www.multiagentcoordinationprotocol.io/docs/determinism
- SWE-agent: https://arxiv.org/abs/2405.15793
- Devin worktrees: https://docs.devin.ai/desktop/cascade/worktrees
- Cursor subagents: https://cursor.com/docs/subagents
- Claude Code worktrees: https://code.claude.com/docs/en/worktrees
- OTel GenAI: https://opentelemetry.io/blog/2026/genai-observability/
- ATSC: https://github.com/agent-telemetry-spec/atsc/blob/main/SPEC.md
- LangSmith coding-agent-v1: https://docs.langchain.com/langsmith/coding-agent-metadata-contract
- AgentOps SDK: https://docs.agentops.ai/v2/usage/sdk-reference
