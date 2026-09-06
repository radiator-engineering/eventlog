//! The model layer: what a log line is, what a type may carry, who may write
//! it, where the log lives, and which paths a claim may name.
//!
//! These signatures are the model contract every later module builds against
//! (`log`, `query`, `react`, `guard`, `scaffold`, `tui`). Spec sections 3 and 4.

pub mod allow;
pub mod config;
pub mod event;
pub mod paths;
pub mod vocab;
