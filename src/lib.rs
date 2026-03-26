//! # Szál — Workflow Engine
//!
//! Szál (Hungarian: thread) provides shared workflow execution for the AGNOS
//! ecosystem. Define steps, wire them into flows with branching, retry, and
//! rollback — then execute sequentially, in parallel, or as a DAG.
//!
//! ## Quick start
//!
//! ```
//! use szal::step::StepDef;
//! use szal::flow::{FlowDef, FlowMode};
//!
//! // Build a DAG pipeline
//! let build = StepDef::new("build");
//! let test = StepDef::new("test").depends_on(build.id);
//! let deploy = StepDef::new("deploy")
//!     .depends_on(test.id)
//!     .with_retries(3, 5_000)
//!     .with_rollback();
//!
//! let mut flow = FlowDef::new("ci-cd", FlowMode::Dag);
//! flow.add_step(build);
//! flow.add_step(test);
//! flow.add_step(deploy);
//! flow.validate().unwrap();
//! ```
//!
//! ## Modules
//!
//! - [`step`] — Individual workflow steps with timeout, retry, rollback, DAG dependencies
//! - [`flow`] — Flow definitions: sequential, parallel, DAG, hierarchical
//! - [`engine`] — Execution configuration and flow result aggregation
//! - [`state`] — Workflow state machine with validated transitions

pub mod bus;
pub mod condition;
pub mod engine;
pub mod flow;
pub mod mcp;
#[cfg(feature = "majra")]
pub mod metrics;
pub mod state;
pub mod step;

mod error;
pub use error::SzalError;

/// Re-export `ai_hwaccel` key types when the `hardware` feature is enabled.
#[cfg(feature = "hardware")]
pub use ai_hwaccel::AcceleratorRequirement;

pub type Result<T> = std::result::Result<T, SzalError>;

#[cfg(test)]
mod tests;
