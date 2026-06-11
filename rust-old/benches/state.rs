use criterion::{Criterion, criterion_group, criterion_main};
use szal::state::WorkflowState;

fn bench_state_transitions(c: &mut Criterion) {
    let all_states = [
        WorkflowState::Created,
        WorkflowState::Running,
        WorkflowState::Paused,
        WorkflowState::Completed,
        WorkflowState::Failed,
        WorkflowState::RollingBack,
        WorkflowState::RolledBack,
        WorkflowState::Cancelled,
    ];

    c.bench_function("state_all_transitions", |b| {
        b.iter(|| {
            let mut valid = 0u32;
            for from in &all_states {
                for to in &all_states {
                    if from.valid_transition(to) {
                        valid += 1;
                    }
                }
            }
            valid
        })
    });
}

criterion_group!(benches, bench_state_transitions);
criterion_main!(benches);
