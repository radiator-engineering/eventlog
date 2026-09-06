# LogAct: Enabling Agentic Reliability via Shared Logs

Balakrishnan et al. (Meta), arXiv:2604.07988v1 [cs.DC], 09 Apr 2026.
https://arxiv.org/html/2604.07988v1

Read for: what this repo's `.context/events.jsonl` CLI/TUI should borrow,
reject, or verify against a paper that builds the same idea — agents acting
through a shared append-only log — at much larger scale and with adversarial
safety in mind.

## 1. The LogAct abstraction

LogAct (§3) turns an agent from an imperative "observe, infer, act" loop into
a state machine that plays a shared log. Four stages, each playing the prior
stage's log entries and appending its own:

- **Inferring.** The Driver reads prior results or user input, calls the LLM,
  and appends an inference output plus an extracted **intent**.
- **Voting.** One or more Voters read the intent, check it for safety, and
  append **votes**. Voters are pluggable: rule-based, LLM-based, or
  simulation-based, and several can run concurrently.
- **Deciding.** The Decider reads the votes, applies a quorum policy
  (`on_by_default`, `first_voter`, `boolean_OR`, `boolean_AND`), and appends
  **commit** or **abort**. On abort, the state machine returns straight to
  Inferring — the Executor never runs.
- **Executing.** The Executor reads a commit, runs the (LLM-generated,
  arbitrary) code against the real environment, and appends a **result**.

These four roles are separate components, not just phases of one process
(§3, "Deconstructed Components"): a Driver, a set of Voters, a Decider, and an
Executor, each independently restartable. The **AgentBus** is the log they
share: a linearizable, durable, typed, access-controlled append log with
`append`, `read`, `tail`, and a blocking `poll(start, filter)` that lets a
component wait for entries of specific types. Entry types are first-class
(`InfIn`, `InfOut`, `Intent`, `Vote`, `Commit`, `Abort`, `Result`, `Mail`,
`Policy`) and access control is enforced per type, not just per log. Two
entry types carry cross-cutting control: **Mail** (external agents or humans
inject messages; if the Driver is idle this triggers a fresh inference, if
it's mid-call the mail buffers for the next round) and **Policy** (only
privileged clients may append these; they reconfigure Driver, Voter, or
Decider behavior in-band, including which voter types count toward quorum).

## 2. Safety guarantees (§3.1) and failure/recovery (§3.2)

**Assumptions.** LogAct assumes nothing about user input, LLM output
(including prompt injection), or the state of the target environment — weaker
than prior work that assumes a "blank-slate" trusted LLM (§3.1). It sorts
components into three trust tiers: **Classic** (no LLM contact — rule-based
voters, static analysis), **LLM-Passive** (reads/writes text but never
executes model-generated code — the Driver, LLM-based voters), and
**LLM-Active** (executes model-generated code — the Executor only). Classic
and LLM-Passive components live on a "safe" tier the Executor's sandbox
cannot reach; LLM-Passive voters have no environment access by default. This
tiering, not the log, is what actually stops a compromised Executor from
tampering with the components that are supposed to police it.

**Formal properties.** Over an ordered sequence of committed intents L and
the resulting environment states E: **Consistency** holds if the logged
intents and the real environment stay in lockstep (every prefix's execution
actually produced the environment claimed). **Safety** holds if every
execution keeps the environment inside true invariants 𝒮.
**Enforced-Safety** is what voters can actually check — an approximation 𝒮̃
of 𝒮 — and is explicitly weaker than true Safety.

**Failure cases** the paper ranks by severity (§3.1):
1. Executor corrupts only the environment (not itself or other components):
   breaks Safety but not Consistency or Enforced-Safety — bad if the voters'
   approximation is far from the true invariant, but at least legible.
2. Executor corrupts itself: can break Consistency — it can lie about what it
   executed, so the log no longer reflects reality even though it still
   satisfies Enforced-Safety.
3. Executor corrupts the Decider or Voters: worst case, since it can rewrite
   Enforced-Safety itself. The paper's only defense is deployment isolation —
   run Executor and Decider/Voters on separate infrastructure with separate
   identities — not anything logical.

**Recovery** is per-component and asymmetric (§3.2):
- **Decider**: classical replicated state machine. State is just the compact
  policy, snapshotted periodically; recovery loads the snapshot and replays
  forward. Multiple Deciders can run at once safely because deciding is
  deterministic — duplicates are just ignored downstream.
- **Driver**: also a clean replicated state machine (conversation history is
  the state), but only one may run at a time. A rebooting Driver appends a
  policy entry that elects itself and fences any prior Driver; downstream
  components reject intents from a fenced Driver. Replay is deterministic
  even though LLM inference isn't, because inference *outputs* were logged,
  not just inputs.
- **Executor**: not a clean state machine — its state spans the process and
  the external environment, which cannot be checkpointed or safely replayed
  (commands aren't idempotent). Recovery is conservative and aims only for
  at-most-once execution: on reboot the Executor appends a special result
  entry, the Driver hands that to the LLM to generate examine/undo/redo
  intents, and those intents go through Voters like anything else — recovery
  gets no safety bypass.
- **Voter**: classical state machine, snapshot + replay. Fencing is simple
  because the Decider's quorum policy names voter *types*, not instances, so
  a restarted voter of the same type just resumes voting.

## 3. Mapping: LogAct concept → this repo

| LogAct | This repo | Match / gap |
|---|---|---|
| AgentBus (typed, access-controlled, linearizable log) | `.context/events.jsonl` (flat JSON lines, one file, no types beyond `type=`) | **Gap.** No entry-type access control — anyone who can write the file can write any `type`. The only enforcement is social (SKILL.md's `by=` convention) plus an optional `chflags uappnd` (`protect-log.sh`), which is append-only at the OS level but not type-scoped. |
| Driver (infers, appends intent) | Controller (spawns workers, appends `spawn`/`prompt`/`decision`) | Partial match. The controller plays the Driver role for orchestration decisions, but there's no `intent` stage before a `decision` — see §5. |
| Voters (check intent before commit) | *(none)* | **Gap.** Nothing in this repo votes on a `decision` or `result` before it takes effect. See §5. |
| Decider (quorum → commit/abort) | *(none, implicit — single controller decides)* | Not needed at current scale: single-writer rule means there's no quorum to compute. Would matter only if this repo grows multiple deciding agents. |
| Executor (plays commit, executes, appends result) | Commit reactor / doc worker (`log-reactors.md`) | Good match. Reactors already play the LogAct pattern: they read `decision`/`result` events and write back their own `ack` — same "outcome is itself an event, not a side file" design LogAct requires for Decider/Driver recovery. |
| `ack` (fields: `by`, `seq_done`, `outcome`, `ref`) | Same as LogAct's per-reactor durable resume point | Match. `log-reactors.md` Rule 1 independently arrives at LogAct's Decider/Driver recovery mechanism: resume point computed from the reactor's own logged acks, never a side file. |
| `claim` (agent, paths) | No LogAct analogue — LogAct has no notion of file ownership, since its Executor acts on an arbitrary environment, not necessarily git | Repo-specific; nothing to reconcile. |
| Reactor lock (`mkdir` pid lock, "one instance") | LogAct's Driver/Decider election via a policy entry that fences prior instances | **Gap, but a cheap one to close.** This repo's `mkdir` lock is a local mutex; LogAct's fencing is a *logged* election, so a fenced-out instance is visible to every reader of the log, not just to the lock file. If this repo ever needs multi-host reactors, mirror LogAct's fencing (append `policy` or `spawn`-like entry that names the winner) instead of relying on filesystem locks alone. |
| Voter isolation tiers (Classic / LLM-Passive / LLM-Active) | Commit reactor described as pure shell (`git add`, `git commit`) with no LLM in the loop | Match by construction: the commit reactor is already "Classic" tier — no LLM contact — which is the strongest tier LogAct offers. Keep it that way; don't let it call an LLM to decide what to stage. |
| `Mail` entries (external nudge, buffered if Driver busy) | `message` event type (`from, to, subject, ref`) | Match. `message` already covers LogAct's Mail semantics (routed, asynchronous); the reactors don't currently "buffer while busy" in the LogAct sense because reactors are stateless pollers, not conversational Drivers — not a gap, just a different consumer shape. |
| `Policy` entries (in-band reconfiguration, access-controlled) | `decision key=log-writers value=...` | Partial match. `decision` events already reconfigure behavior in-band (who may write `by=`), but there's no access control on who may *append* a `decision` — anyone with `append-event.sh` can. LogAct restricts Policy appends to privileged clients. |

## 4. Recommendations, ranked

Each tagged **borrow** / **reject** / **verify**, with the paper's own
evidence or stated limitation as the reason.

1. **Borrow: keep resuming reactors from their own logged `ack`, never a side
   file.** Already the design (`log-reactors.md` Rule 1); LogAct arrives at
   the identical mechanism independently for Decider and Driver recovery
   (§3.2) and gives it a name — this is the load-bearing idea, not a nice-to-
   have. No change needed; cite this paper in `log-reactors.md` as
   independent confirmation.

2. **Borrow: typed access control on log entries, scoped to this repo's
   `decision` and `result` types.** LogAct restricts who may write which
   entry type (§3, "AgentBus"; §3.1 Policy access control), and the paper's
   own failure-case ranking (§3.1, Case 3) says the worst failure is an
   untrusted component overwriting what the trusted components rely on —
   exactly the risk of an unconstrained `by=` field. Concretely: extend
   `append-event.sh` to check the acting agent's name against a per-type
   allowlist (e.g., only the controller may write `decision` or `spawn`;
   named reactors may only write `ack`) instead of relying on the social
   convention plus `chflags`. This is cheap relative to the paper's tiering
   and closes a real gap in row 1 and row 9 of the table above.

3. **Verify, don't blindly borrow: a pre-commit `intent` + voter stage for
   the commit reactor.** See §5 below for the full cost/benefit — this is
   the brief's specific question and deserves its own section, not a one-
   line verdict.

4. **Reject (at current scale): a Decider / quorum layer.** LogAct's Decider
   exists because it supports multiple concurrent Voters and needs a
   deterministic way to combine their votes (§3, `boolean_OR`/`boolean_AND`
   policies). This repo has a single controller and single-writer rule by
   design (`EVENTLOG.md`, "Single writer"). Adding a Decider would be
   solving a coordination problem this repo doesn't have yet. Revisit only
   if this repo grows multiple agents that can independently propose
   `decision`s that need reconciling.

5. **Verify: fencing via a logged election, not just a `mkdir` lock, before
   this repo runs reactors across more than one host.** The paper's Driver
   recovery (§3.2) fences old instances by writing a policy entry that
   downstream components check against, which survives host loss in a way a
   local pid-file lock cannot. Today's single-host `mkdir` lock
   (`log-reactors.md` Rule 2) is adequate for the paper's own Rule-2 checklist
   ("start a second instance, it must refuse") as long as reactors stay
   single-host. Worth verifying against the checklist in `log-reactors.md`
   before any move to remote/distributed reactors, not before.

6. **Borrow with caution: "stage only what the decision names."** LogAct
   doesn't have a git-staging analogue, but its Executor-recovery principle —
   never let a recovering component take an action broader than what the log
   authorized (§3.2, Executor recovery: undo/redo intents still go through
   Voters, recovery gets no bypass) — is the same reasoning already written
   into `log-reactors.md` Rule 4 (stage only `paths=`, never `git add -A`).
   No new work; note it as validated by the paper's independent design.

7. **Reject: adopting LogAct's multi-tier trust model (Classic /
   LLM-Passive / LLM-Active) as formal repo policy.** The paper itself flags
   this as evaluated only for single-agent safety, with multi-agent
   coordination "left for future work" (§7, Key Limitations). This repo's
   commit reactor is already Classic-tier by construction (row 7 of the
   table). Formalizing three tiers for a two-reactor repo is overhead the
   evidence doesn't yet justify — reconsider if a future reactor calls an
   LLM to decide what to act on.

## 5. Intention before execution: does a pre-commit `intent` + voter fit the commit reactor?

**What it would look like.** Before the controller's `decision` (or a
worker's `result`) takes effect, append an `intent` event naming the proposed
action (e.g., `intent by=controller paths=foo.rs,bar.rs summary="..."`), have
one or more voters (a rule-based check — do the paths match an active
`claim`? is the summary non-empty? — and optionally an LLM-based check for
riskier actions) append `vote` events, then only have the commit reactor act
once a `commit` event appears. Structurally this is a direct lift of LogAct's
Inferring → Voting → Deciding → Executing pipeline (§3) onto the existing
`decision`/`result` → commit-reactor → `ack` pipeline.

**What it would cost.**
- A new mandatory hop on every controller action: `decision`/`result` →
  `intent` → `vote` → `commit` → act → `ack`, versus today's `decision`/
  `result` → act → `ack`. The paper's own overhead numbers (§5.1, §5.2) are
  the right yardstick: adding a single rule-based voter cost 58% extra
  latency (10.6s vs. 6.7s) in their benchmark, and a second (LLM) voter
  added another 15% (12.2s vs. 10.6s) — and that's on a system already
  paying LLM-inference latency per stage. This repo's commit reactor is a
  fast shell script (`git add`/`git commit`) with none of that inference
  latency baseline, so the same absolute voter overhead would dominate the
  reactor's runtime far more than it dominates LogAct's.
- A rule-based voter here is cheap to justify: it can just check "does
  `paths=` match a live `claim=`" — a fact already loggable — which is
  exactly the Classic tier (no LLM contact) LogAct recommends for this kind
  of check (§3.1). That piece is low-cost and plausibly worth adding.
  Practically: it's the automatable half of Rule 4's manual `git show --stat`
  check.
- An LLM-based voter is not obviously worth it here. LogAct needed one
  because its dual-voter setup restored utility lost to a too-strict
  rule-based voter (§5.2: rule-based alone dropped utility to 49.5% from an
  81.4% baseline; adding the LLM voter recovered it to 78.4%). This repo's
  commit reactor doesn't have a utility-vs-safety tradeoff of that shape —
  the actions it takes (`git add` + `git commit` of named paths) are narrow
  and already constrained by `paths=`, so there's little for a rule-based
  voter to over-block that an LLM voter would need to rescue.
- The bigger cost is conceptual, not computational: it adds a Decider-shaped
  role (something that reads votes and computes commit/abort) to a repo
  whose single-writer design explicitly avoids needing one (recommendation
  4 above). Introducing `intent`/`vote`/`commit` without also deciding who
  runs the Decider role reintroduces the coordination problem LogAct built
  a whole component to solve.

**Verdict: verify, scoped down.** Add a single Classic-tier rule check
("does this `decision`'s `paths=` match a live `claim`, and is `paths=`
non-empty") directly inside the commit reactor as a guard before it stages
anything — this captures the safety benefit LogAct's cheapest voter provides,
at effectively zero added latency, without introducing a separate `intent`
event, a separate `vote` event, or a Decider role this repo doesn't need. Do
not adopt the full four-stage pipeline unless a specific incident shows the
guard-inside-the-reactor approach missed something a standalone `intent`
stage would have caught.

## 6. Related work to read next (§6)

- **CORFU, Tango, Delos, FuzzyLog** (shared-log systems the paper says
  AgentBus "builds on... adding entry types and type-granular access
  control"). Read to understand what a production shared-log API looks like
  at scale, since `events.jsonl` is a much smaller instance of the same idea.
  Tango and Delos specifically propose state machines sharing a log, but as
  independent objects/layers rather than deconstructed pieces of one agent —
  worth reading to see why LogAct's decomposition differs.
- **Camel** (cited for "dual-LLM separation" between generation and
  execution). Directly relevant to recommendation 7 / §5: it's the closest
  prior work to LogAct's Classic/LLM-Passive/LLM-Active tiering, done with
  two LLMs instead of a log.
- **LlamaFirewall** (cited for "pluggable safety verifiers without full
  system design"). Read as the narrower predecessor to LogAct's Voters —
  useful if this repo ever builds the rule-based voter from §5 and wants
  prior art on verifier design specifically, without needing the rest of the
  log machinery.

## Not covered

The paper's §4 (three AgentBus backends: in-memory, SQLite, disaggregated)
and §5.3–5.4 (semantic recovery giving a 290x speedup on a stuck sub-agent;
swarm introspection giving 17% more work for 41% fewer tokens) are evaluation
results about performance and multi-agent swarms, not safety or reliability
guarantees this repo's two-reactor setup needs today. Worth a second pass if
this repo grows toward supervising many parallel workers, since §5.4's
"supervisor as gossip hub" result is a direct answer to how a controller
should summarize progress across many spawned workers instead of relaying
everything.
