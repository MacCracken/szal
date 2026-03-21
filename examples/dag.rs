//! DAG workflow with diamond dependency pattern.

use szal::flow::{FlowDef, FlowMode};
use szal::step::StepDef;

fn main() {
    //       build
    //      /     \
    //  unit-test  integ-test
    //      \     /
    //       deploy
    let build = StepDef::new("build");
    let unit_test = StepDef::new("unit-test").depends_on(build.id);
    let integ_test = StepDef::new("integ-test").depends_on(build.id);
    let deploy = StepDef::new("deploy")
        .depends_on(unit_test.id)
        .depends_on(integ_test.id)
        .with_rollback();

    let mut flow = FlowDef::new("ci-cd", FlowMode::Dag).with_rollback();
    flow.add_step(build);
    flow.add_step(unit_test);
    flow.add_step(integ_test);
    flow.add_step(deploy);

    flow.validate().unwrap();
    println!("DAG flow '{}' validated: {} steps", flow.name, flow.steps.len());
    for step in &flow.steps {
        println!(
            "  - {} (deps={})",
            step.name,
            step.depends_on.len()
        );
    }
}
