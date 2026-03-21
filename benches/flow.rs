use criterion::{Criterion, criterion_group, criterion_main};
use szal::flow::{FlowDef, FlowMode};
use szal::step::StepDef;

fn bench_flow_serde(c: &mut Criterion) {
    let mut flow = FlowDef::new("bench-flow", FlowMode::Dag);
    let mut prev_id = None;
    for i in 0..50 {
        let mut step = StepDef::new(format!("step-{i}"));
        if let Some(pid) = prev_id {
            step = step.depends_on(pid);
        }
        prev_id = Some(step.id);
        flow.add_step(step);
    }
    let json = serde_json::to_string(&flow).unwrap();

    c.bench_function("flow_50_serialize", |b| {
        b.iter(|| serde_json::to_string(&flow).unwrap())
    });
    c.bench_function("flow_50_deserialize", |b| {
        b.iter(|| serde_json::from_str::<FlowDef>(&json).unwrap())
    });
}

fn bench_dag_validation_wide(c: &mut Criterion) {
    // Fan-out: one root, 100 leaves
    let root = StepDef::new("root");
    let root_id = root.id;
    let mut flow = FlowDef::new("wide-dag", FlowMode::Dag);
    flow.add_step(root);
    for i in 0..100 {
        flow.add_step(StepDef::new(format!("leaf-{i}")).depends_on(root_id));
    }

    c.bench_function("dag_validate_wide_100", |b| {
        b.iter(|| flow.validate().unwrap())
    });
}

criterion_group!(benches, bench_flow_serde, bench_dag_validation_wide);
criterion_main!(benches);
