//! # Szál — Workflow Engine
//!
//! Szál (Hungarian: thread) provides shared workflow execution for the AGNOS
//! ecosystem. Define steps, wire them into flows with branching, retry, and
//! rollback — then execute sequentially, in parallel, or as a DAG.
//!
//! ## Modules
//!
//! - [`step`] — Individual workflow steps with input/output types
//! - [`flow`] — Flow definitions: sequential, parallel, conditional, DAG
//! - [`engine`] — Execution runtime with retry, timeout, rollback
//! - [`state`] — Workflow state machine and persistence

pub mod engine;
pub mod flow;
pub mod state;
pub mod step;

mod error;
pub use error::SzalError;

pub type Result<T> = std::result::Result<T, SzalError>;

#[cfg(test)]
mod tests;
