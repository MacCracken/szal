//! Workflow state machine transitions.

use szal::state::WorkflowState;

fn main() {
    let transitions = [
        (WorkflowState::Created, WorkflowState::Running),
        (WorkflowState::Running, WorkflowState::Paused),
        (WorkflowState::Paused, WorkflowState::Running),
        (WorkflowState::Running, WorkflowState::Completed),
        (WorkflowState::Running, WorkflowState::Failed),
        (WorkflowState::Failed, WorkflowState::RollingBack),
        (WorkflowState::RollingBack, WorkflowState::RolledBack),
        // Invalid transitions
        (WorkflowState::Completed, WorkflowState::Running),
        (WorkflowState::Created, WorkflowState::Completed),
    ];

    for (from, to) in &transitions {
        let valid = from.valid_transition(to);
        let terminal = to.is_terminal();
        println!(
            "{} -> {} : {} {}",
            from,
            to,
            if valid { "VALID" } else { "INVALID" },
            if terminal { "(terminal)" } else { "" }
        );
    }
}
