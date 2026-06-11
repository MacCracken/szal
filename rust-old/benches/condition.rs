use criterion::{Criterion, criterion_group, criterion_main};
use serde_json::json;
use szal::condition::{CompiledCondition, ConditionCache, evaluate};

/// A representative non-trivial condition with paths, comparisons, and boolean logic.
const EXPR: &str =
    "steps.build.status == 'completed' && steps.test.exit_code == 0 || steps.deploy.forced";

fn ctx() -> serde_json::Value {
    json!({
        "steps": {
            "build": { "status": "completed" },
            "test": { "exit_code": 0 },
            "deploy": { "forced": false }
        }
    })
}

/// Baseline: `evaluate` re-tokenizes and re-parses the expression on every call.
fn bench_evaluate_uncached(c: &mut Criterion) {
    let context = ctx();
    c.bench_function("cond_uncached", |b| {
        b.iter(|| evaluate(std::hint::black_box(EXPR), &context).unwrap())
    });
}

/// Cached path: the expression is compiled once and looked up by string thereafter.
fn bench_evaluate_cached(c: &mut Criterion) {
    let context = ctx();
    let cache = ConditionCache::new();
    // Warm the cache so we measure the steady-state lookup + eval cost.
    let _ = cache.evaluate(EXPR, &context);
    c.bench_function("cond_cached", |b| {
        b.iter(|| {
            cache
                .evaluate(std::hint::black_box(EXPR), &context)
                .unwrap()
        })
    });
}

/// Pre-compiled path: no cache lookup, pure AST evaluation.
fn bench_evaluate_compiled(c: &mut Criterion) {
    let context = ctx();
    let compiled = CompiledCondition::compile(EXPR).unwrap();
    c.bench_function("cond_compiled", |b| {
        b.iter(|| compiled.evaluate(std::hint::black_box(&context)).unwrap())
    });
}

criterion_group!(
    benches,
    bench_evaluate_uncached,
    bench_evaluate_cached,
    bench_evaluate_compiled
);
criterion_main!(benches);
