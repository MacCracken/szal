use criterion::{Criterion, criterion_group, criterion_main};
use szal::step::StepDef;

fn bench_step_builder(c: &mut Criterion) {
    c.bench_function("step_builder_full", |b| {
        b.iter(|| {
            let s1 = StepDef::new("build");
            StepDef::new("deploy")
                .with_timeout(60_000)
                .with_retries(3, 5_000)
                .with_rollback()
                .depends_on(s1.id)
        })
    });
}

fn bench_step_serde(c: &mut Criterion) {
    let step = StepDef::new("test-step")
        .with_timeout(60_000)
        .with_retries(3, 5_000)
        .with_rollback();
    let json = serde_json::to_string(&step).unwrap();

    c.bench_function("step_serialize", |b| {
        b.iter(|| serde_json::to_string(&step).unwrap())
    });
    c.bench_function("step_deserialize", |b| {
        b.iter(|| serde_json::from_str::<StepDef>(&json).unwrap())
    });
}

criterion_group!(benches, bench_step_builder, bench_step_serde);
criterion_main!(benches);
