//! Sequential deployment pipeline.

use szal::flow::{FlowDef, FlowMode};
use szal::step::StepDef;

fn main() {
    let mut flow = FlowDef::new("deploy-pipeline", FlowMode::Sequential);
    flow.add_step(StepDef::new("build").with_timeout(60_000));
    flow.add_step(
        StepDef::new("test")
            .with_retries(2, 1_000)
            .with_timeout(120_000),
    );
    flow.add_step(StepDef::new("deploy").with_rollback());

    flow.validate().unwrap();
    println!("Flow '{}' validated: {} steps, mode={}", flow.name, flow.steps.len(), flow.mode);
    for step in &flow.steps {
        println!(
            "  - {} (timeout={}ms, retries={}, rollback={})",
            step.name, step.timeout_ms, step.max_retries, step.rollbackable
        );
    }
}
