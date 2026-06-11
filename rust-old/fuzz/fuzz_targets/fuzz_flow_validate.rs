#![no_main]
use libfuzzer_sys::fuzz_target;
use szal::flow::{FlowDef, FlowMode};
use szal::step::StepDef;

/// Fuzz flow validation with random step counts and dependency wiring.
fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }
    let step_count = (data[0] as usize % 32) + 1;
    let mode = match data[1] % 4 {
        0 => FlowMode::Sequential,
        1 => FlowMode::Parallel,
        2 => FlowMode::Dag,
        _ => FlowMode::Hierarchical,
    };

    let mut flow = FlowDef::new("fuzz", mode);
    let mut step_ids = Vec::new();

    for i in 0..step_count {
        let mut step = StepDef::new(format!("s{i}"));
        // Only wire dependencies for DAG mode — non-DAG flows with deps always fail validation
        let data_idx = 2 + i;
        if mode == FlowMode::Dag && data_idx < data.len() && !step_ids.is_empty() {
            let dep_idx = data[data_idx] as usize % step_ids.len();
            step = step.depends_on(step_ids[dep_idx]);
        }
        step_ids.push(step.id);
        flow.add_step(step);
    }

    // validate() must never panic regardless of input
    let _ = flow.validate();
});
