# Research

Reports that back the design of the event-log CLI. Earlier recon from the
session that created the repo lives in `.context/research/`; start there for
the skill inventory and the herdr API notes.

| File | Author | What it answers |
|---|---|---|
| `neuroarxiv-report.md` | controller, `neuroarxiv` skill | arXiv prior art, one recommended path (log the intention before a reactor acts), pitfalls |
| `logact-2604.07988.md` | `logact-deep-read` (Sonnet) | Full read of LogAct and a mapping onto this repo's concepts |
| `survey-event-sourcing.md` | `survey-event-sourcing` (cursor-agent) | Event sourcing, log durability, acks, tamper evidence, concurrency, retention |
| `survey-agent-coordination.md` | `survey-agent-coordination` (cursor-agent) | Multi-agent coordination research and practice, protocols, audit, failure modes |

Every report ends with a ranked table: recommendation, source, adopt / defer /
reject, reason. Disagreements between the tables are design questions for
the spec, not errors.
