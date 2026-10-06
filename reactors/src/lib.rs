//! Optional reactors for an `eventlog` coordination log: long-running agents
//! that wait for an event type and act on it (commit a `result`'s paths,
//! document what was committed). The log works without them; a project opts
//! in with `eventlog-reactors setup apply`.

pub mod cli;
pub mod cmd;
pub mod react;
pub mod setup;
