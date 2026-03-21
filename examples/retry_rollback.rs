//! Configuring retry and rollback behavior.

use szal::flow::{FlowDef, FlowMode};
use szal::step::StepDef;

fn main() {
    let mut flow = FlowDef::new("resilient-pipeline", FlowMode::Sequential)
        .with_rollback()
        .with_timeout(300_000);

    flow.add_step(
        StepDef::new("provision-infra")
            .with_timeout(120_000)
            .with_retries(3, 5_000)
            .with_rollback(),
    );
    flow.add_step(
        StepDef::new("run-migrations")
            .with_retries(2, 10_000)
            .with_rollback(),
    );
    flow.add_step(
        StepDef::new("deploy-app")
            .with_retries(3, 5_000)
            .with_rollback(),
    );
    flow.add_step(StepDef::new("smoke-test").with_retries(2, 3_000));

    flow.validate().unwrap();
    println!(
        "Flow '{}': {} steps, rollback_on_failure={}, timeout={:?}ms",
        flow.name,
        flow.steps.len(),
        flow.rollback_on_failure,
        flow.timeout_ms
    );
    for step in &flow.steps {
        println!(
            "  - {} (retries={}, delay={}ms, rollback={})",
            step.name, step.max_retries, step.retry_delay_ms, step.rollbackable
        );
    }
}
