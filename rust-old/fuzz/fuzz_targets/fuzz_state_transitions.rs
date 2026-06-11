#![no_main]
use libfuzzer_sys::fuzz_target;
use szal::state::WorkflowState;

const STATES: [WorkflowState; 8] = [
    WorkflowState::Created,
    WorkflowState::Running,
    WorkflowState::Paused,
    WorkflowState::Completed,
    WorkflowState::Failed,
    WorkflowState::RollingBack,
    WorkflowState::RolledBack,
    WorkflowState::Cancelled,
];

/// Walk a random sequence of state transitions — must never panic.
fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let mut current = STATES[data[0] as usize % STATES.len()];
    for &byte in &data[1..] {
        let target = STATES[byte as usize % STATES.len()];
        let valid = current.valid_transition(&target);
        let _ = current.is_terminal();
        let _ = current.to_string();
        if valid {
            current = target;
        }
    }
});
