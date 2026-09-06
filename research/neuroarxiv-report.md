# NeuroArxiv pass: prior art for the event-log CLI

Run on 2026-09-06 by the controller with the `neuroarxiv` skill. Real arXiv
export-API fetches, one isolated read per paper, then one recommendation.
Seed paper supplied by the user: LogAct (arXiv 2604.07988).

## 1. Searched

| Category | Terms | Hits | Read |
|---|---|---|---|
| cs.MA | shared log, event log, LLM agents + coordination | 4 | 1 |
| cs.DC | shared log, append-only log, state machine replication + agents | 4 | 4 |
| cs.SE | coding agents, multi-agent + software engineering + coordination | 4 | 3 |
| cs.CR | LLM agents + audit log / tamper-evident / prompt injection defense | 4 | 2 |
| cs.AI | LLM agents + replay / checkpoint / human-in-the-loop approval | 4 | 1 |

20 papers fetched, 11 read. Dropped as off-topic: opponent shaping, device-cloud
coordination, social ties, commit-history reconstruction, three RL
experience-replay papers, and two prompt-structure defenses.

## 2. Papers read, by cluster

Score chips: `rel` fit to this build, `prac` buildable by a small team,
`rig` evidence in the abstract itself. 0 to 10.

**A. Log-before-execute (the chosen cluster)**
- 2604.07988 LogAct: Enabling Agentic Reliability via Shared Logs (Meta).
  Each agent is a state machine replaying a shared log; actions are appended
  before execution and independent voters can veto them; the log is
  introspected for recovery and health checks. `[rel9 prac7 rig8]`

**B. Branchable logs**
- 2604.14590 AgileLog: A Forkable Shared Log for Agents on Data Streams.
  A shared log that forks itself so a risky agent task writes to an isolated
  branch; copy-on-write keeps forks cheap. `[rel5 prac5 rig7]`

**C. Adversarial replication and AI validators**
- 2307.08381 2P-BFT-Log. Two-phase single-author append-only log; replicas
  converge on the latest valid message or on evidence of a fork. `[rel3 prac4 rig7]`
- 2607.16109 The Honest Quorum Problem (EBFT). AI validators can endorse
  semantically invalid transitions; confidence-indexed thresholds bound the
  damage. `[rel3 prac3 rig5]`

**D. Role and organization frameworks**
- 2602.01465 Agyn. Coordinator, researcher, implementer, reviewer roles;
  72.2 percent on SWE-bench 500. `[rel4 prac5 rig7]`
- 2602.04418 SPEAR. Planning, execution and repair agents that negotiate and
  revise local beliefs (AGM). `[rel3 prac4 rig5]`
- 2608.11965 MAS-in-SE experience report. Frameworks cover the basics;
  telemetry of agents is still missing. `[rel4 prac5 rig6]`

**E. Limits of guards under adaptive attack**
- 2503.00061 Adaptive Attacks Break Defenses Against Indirect Prompt
  Injection. Eight defenses, all bypassed, attack success above 50 percent.
  `[rel4 prac7 rig8]`
- 2504.19793 ToolHijacker. One poisoned tool document steers tool selection;
  prevention and detection defenses are insufficient. `[rel3 prac5 rig7]`

**F. Framing and post-hoc analysis**
- 2604.08224 Externalization in LLM Agents (review). Memory, skills and
  protocols move out of the weights into shared infrastructure. `[rel5 prac4 rig4]`
- 2606.08275 Causal Agent Replay. Counterfactual re-execution to find the
  step that caused a failure; LLM-judge attribution is about 14 percent
  accurate. `[rel4 prac4 rig5]`

## 3. Prior-art pitfalls (watch-outs, not verdicts)

- **Voters approximate safety.** LogAct says an unsafe intention still runs
  when a voter is wrong. A rule-based voter cut attack success to 1.4 percent
  but also cut benign utility to 49.5 percent; adding an LLM voter recovered
  78.4 percent. Expect the same trade-off in any commit gate.
- **Guards fall to adaptive attackers.** Every published injection defense in
  2503.00061 was bypassed by an attack built against it. Test the PreToolUse
  guard and the OS flag with inputs crafted against them, not with a fixed
  list of bad commands.
- **Registration is an attack surface.** ToolHijacker shows one injected
  document can steer selection. Any `spawn` or `claim` that names a new agent
  or path is untrusted input to a reactor.
- **AI validators fail semantically while obeying the protocol** (EBFT). An
  LLM committer that follows every rule can still commit the wrong thing.
- **Post-hoc blame is unreliable.** Causal Agent Replay reports about 14
  percent step-level accuracy for LLM-judge attribution. The log should
  record intent at the step that decided, not only the step that acted.
- **Telemetry is the missing feature** in current MAS frameworks (2608.11965).
  It is a gap this tool fills, not one it can borrow.

## 4. THE PATH: log the intention before the reactor acts

**Chosen cluster: A, log-before-execute (LogAct).** It is the only paper
that addresses this exact shape: a shared append-only log that drives
autonomous effects, with the effect recorded before it happens.

**Implementation sketch.** Add two event types to the vocabulary: `intent`
and `veto`. A reactor appends `intent by=<reactor> for=<seq> action=<what>
paths=<list>` before it runs the action, then waits for a bounded veto
window. In that window a deterministic voter, built into the CLI, checks the
intent against the log: every path in `paths=` is covered by a live `claim`
or by the driving event's own `paths=`, nothing under `.context/`, no path
already claimed by another open agent, and no open `escalate` for this
reactor. If the check fails, the CLI appends `veto by=voter for=<intent seq>
reason=<rule>` and the reactor acks `outcome=vetoed`. The controller or a
human can append the same `veto` inside the window. Execution then follows
the existing `ack seq_done=` pattern, so resume logic is unchanged: a reactor
that restarts replays unacked intents and re-checks them. The TUI shows open
intents as a column beside open claims. This is LogAct's driver, voter,
decider and executor split, with the decider collapsed into "no veto within
the window" because this log has one writer and no quorum.

**Citations.**
- 2604.07988 LogAct, https://arxiv.org/abs/2604.07988, primary mechanism.
- 2503.00061 Adaptive attacks, https://arxiv.org/abs/2503.00061, failure
  mode to avoid: test the voter with adaptive inputs.
- 2504.19793 ToolHijacker, https://arxiv.org/abs/2504.19793, failure mode to
  avoid: treat registration events as untrusted.
- 2607.16109 EBFT, https://arxiv.org/abs/2607.16109, supporting evidence
  that a rule-following AI step can still be semantically wrong.
- 2606.08275 Causal Agent Replay, https://arxiv.org/abs/2606.08275,
  supporting evidence that the deciding step must be logged.

**First concrete step.** Add `intent` and `veto` to `.context/EVENTLOG.md`
and make `cursor-commit-reactor.sh` append an `intent` naming the driving
seq and its `paths=` before it invokes `cursor-agent`, with a rule-based
check and a `veto` on failure. No new process, no LLM voter. Measure how many
passes get vetoed over a week.

**Load-bearing risk.** The veto window adds latency to every reactor pass,
and a window with no explicit "approve" means silence is consent. If the
controller is busy or dead, nothing vetoes. Keep the window short (seconds)
and keep the rule-based voter in the reactor process, not in the controller.

**Avoid.**
- Do not rely on an LLM voter alone; LogAct's numbers show it trades safety
  for utility, and 2503.00061 shows it can be steered.
- Do not add forkable log branches (AgileLog); this log has one writer and
  no merge step, so a branch has nothing to isolate.
- Do not add Byzantine replication or validator quorums (2P-BFT-Log, EBFT);
  the trust model is cooperating agents on one machine.
- Do not trust `spawn`, `claim` or a tool list advertised in a prompt without
  a decision or allowlist behind it (ToolHijacker).
- Do not expect post-hoc replay to explain failures; record intent up front
  (Causal Agent Replay).
- Do not adopt a MAS framework for the coordination layer; the experience
  report finds their telemetry missing, which is the part this tool provides.

## 5. Alternates considered, not chosen

- **B, branchable logs.** Real engineering (Bolt) but it solves multi-writer
  isolation, which this design avoids on purpose.
- **C, adversarial replication and AI validators.** Right worry, wrong trust
  model: no replicas, no quorum, one trusted writer.
- **D, role frameworks.** They shape who does what, which AGENTS.md already
  settles; none of them speaks to log semantics.
- **E, guard limits.** Not an architecture, but its lesson is folded into the
  path: adaptive tests for the guard and the voter.
- **F, framing and post-hoc replay.** Confirms the direction (log as
  externalized memory) and warns that attribution after the fact is weak.

## 6. Open thread

LogAct assumes each agent has exclusive access to its environment or a
concurrency control protocol. This repo's reactors share one worktree with
the controller and the workers. What is the concurrency protocol for an
`intent` whose `paths=` overlap a file the controller is editing at that
moment? The claims model covers workers; it does not yet cover the
controller's own uncommitted edits. Decide this before building the voter.
