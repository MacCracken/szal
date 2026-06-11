use criterion::{Criterion, criterion_group, criterion_main};
use szal::flow::{FlowDef, FlowMode};
use szal::step::StepDef;

fn bench_dag_validation(c: &mut Criterion) {
    let mut flow = FlowDef::new("bench", FlowMode::Dag);
    let mut prev_id = None;
    for i in 0..100 {
        let mut step = StepDef::new(format!("step-{i}"));
        if let Some(pid) = prev_id {
            step = step.depends_on(pid);
        }
        prev_id = Some(step.id);
        flow.add_step(step);
    }
    c.bench_function("dag_validate_100_steps", |b| {
        b.iter(|| flow.validate().unwrap())
    });
}

criterion_group!(benches, bench_dag_validation);
criterion_main!(benches);
