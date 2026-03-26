use criterion::{Criterion, criterion_group, criterion_main};
use serde_json::json;
use szal::engine::{Engine, EngineConfig, handler_fn};
use szal::flow::{FlowDef, FlowMode};
use szal::step::StepDef;

fn noop_engine() -> Engine {
    Engine::new(
        EngineConfig::default(),
        handler_fn(|_step| async move { Ok(json!(null)) }),
    )
}

fn make_sequential(n: usize) -> FlowDef {
    let mut flow = FlowDef::new("bench-seq", FlowMode::Sequential);
    for i in 0..n {
        flow.add_step(StepDef::new(format!("s{i}")));
    }
    flow
}

fn make_parallel(n: usize) -> FlowDef {
    let mut flow = FlowDef::new("bench-par", FlowMode::Parallel);
    for i in 0..n {
        flow.add_step(StepDef::new(format!("s{i}")));
    }
    flow
}

fn make_dag_diamond() -> FlowDef {
    let build = StepDef::new("build");
    let test_a = StepDef::new("test-a").depends_on(build.id);
    let test_b = StepDef::new("test-b").depends_on(build.id);
    let deploy = StepDef::new("deploy")
        .depends_on(test_a.id)
        .depends_on(test_b.id);
    let mut flow = FlowDef::new("bench-dag-diamond", FlowMode::Dag);
    flow.add_step(build);
    flow.add_step(test_a);
    flow.add_step(test_b);
    flow.add_step(deploy);
    flow
}

fn make_dag_linear(n: usize) -> FlowDef {
    let mut flow = FlowDef::new("bench-dag-linear", FlowMode::Dag);
    let mut prev_id = None;
    for i in 0..n {
        let mut step = StepDef::new(format!("s{i}"));
        if let Some(pid) = prev_id {
            step = step.depends_on(pid);
        }
        prev_id = Some(step.id);
        flow.add_step(step);
    }
    flow
}

fn make_hierarchical(managers: usize, children: usize) -> FlowDef {
    let mut flow = FlowDef::new("bench-hier", FlowMode::Hierarchical);
    for m in 0..managers {
        let mut manager = StepDef::new(format!("mgr{m}"));
        for c in 0..children {
            manager = manager.with_sub_step(StepDef::new(format!("mgr{m}-child{c}")));
        }
        flow.add_step(manager);
    }
    flow
}

fn bench_engine(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let engine = noop_engine();

    let seq10 = make_sequential(10);
    let seq100 = make_sequential(100);
    let par10 = make_parallel(10);
    let par100 = make_parallel(100);
    let dag_diamond = make_dag_diamond();
    let dag_linear = make_dag_linear(100);
    let hier10x10 = make_hierarchical(10, 10);

    c.bench_function("engine_sequential_10", |b| {
        b.iter(|| rt.block_on(engine.run(&seq10)).unwrap())
    });
    c.bench_function("engine_sequential_100", |b| {
        b.iter(|| rt.block_on(engine.run(&seq100)).unwrap())
    });
    c.bench_function("engine_parallel_10", |b| {
        b.iter(|| rt.block_on(engine.run(&par10)).unwrap())
    });
    c.bench_function("engine_parallel_100", |b| {
        b.iter(|| rt.block_on(engine.run(&par100)).unwrap())
    });
    c.bench_function("engine_dag_diamond", |b| {
        b.iter(|| rt.block_on(engine.run(&dag_diamond)).unwrap())
    });
    c.bench_function("engine_dag_linear_100", |b| {
        b.iter(|| rt.block_on(engine.run(&dag_linear)).unwrap())
    });
    c.bench_function("engine_hierarchical_10x10", |b| {
        b.iter(|| rt.block_on(engine.run(&hier10x10)).unwrap())
    });
}

criterion_group!(benches, bench_engine);
criterion_main!(benches);
