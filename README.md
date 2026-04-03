# Szál

> Szál (Hungarian: thread) — Workflow orchestration engine for Rust

[![Crates.io](https://img.shields.io/crates/v/szal.svg)](https://crates.io/crates/szal)
[![CI](https://github.com/MacCracken/szal/actions/workflows/ci.yml/badge.svg)](https://github.com/MacCracken/szal/actions/workflows/ci.yml)
[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)

Define steps, wire them into flows with branching, retry, and rollback — then execute sequentially, in parallel, or as a DAG. Part of the [AGNOS](https://github.com/MacCracken) ecosystem.

## Quick start

```toml
[dependencies]
szal = "1"
```

### Sequential flow

```rust
use szal::flow::{FlowDef, FlowMode};
use szal::step::StepDef;

let mut flow = FlowDef::new("deploy-pipeline", FlowMode::Sequential);
flow.add_step(StepDef::new("build").with_timeout(60_000));
flow.add_step(StepDef::new("test").with_retries(2, 1_000));
flow.add_step(StepDef::new("deploy").with_rollback());
flow.validate().unwrap();
```

### DAG workflow

```rust
use szal::flow::{FlowDef, FlowMode};
use szal::step::StepDef;

let build = StepDef::new("build");
let unit_test = StepDef::new("unit-test").depends_on(build.id);
let integ_test = StepDef::new("integ-test").depends_on(build.id);
let deploy = StepDef::new("deploy")
    .depends_on(unit_test.id)
    .depends_on(integ_test.id);

let mut flow = FlowDef::new("ci-cd", FlowMode::Dag);
flow.add_step(build);
flow.add_step(unit_test);
flow.add_step(integ_test);
flow.add_step(deploy);
flow.validate().unwrap(); // cycle detection + dependency validation
```

### State machine

```rust
use szal::state::WorkflowState;

let state = WorkflowState::Created;
assert!(state.valid_transition(&WorkflowState::Running));
assert!(!state.is_terminal());
```

## Modules

| Module | Description |
|--------|-------------|
| `step` | Atomic workflow steps — timeout, retry, rollback, DAG dependencies |
| `flow` | Flow composition — sequential, parallel, DAG (Kahn's), hierarchical |
| `engine` | Execution config and flow result aggregation |
| `state` | Workflow state machine with validated transitions |
| `error` | Typed errors — step failure, timeout, retry exhaustion, cycle detection |

## Execution modes

| Mode | Description |
|------|-------------|
| `Sequential` | Steps run one after another |
| `Parallel` | Steps run concurrently (no dependencies) |
| `Dag` | Dependency graph with cycle detection (DFS) |
| `Hierarchical` | Manager step delegates to sub-steps |

## Roadmap

| Version | Milestone | Status |
|---------|-----------|--------|
| **1.1** | Persistent state, flow composition, streaming output, richer condition DSL | Current |
| **1.2** | Persistent backends, flow versioning, distributed DAG, WebSocket streaming | Planned |

## License

AGPL-3.0-only
