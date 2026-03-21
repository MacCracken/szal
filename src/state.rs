//! Workflow state machine and persistence.

use serde::{Deserialize, Serialize};

/// Workflow execution state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowState {
    Created,
    Running,
    Paused,
    Completed,
    Failed,
    RollingBack,
    RolledBack,
    Cancelled,
}

impl WorkflowState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::RolledBack | Self::Cancelled
        )
    }

    pub fn valid_transition(&self, to: &Self) -> bool {
        matches!(
            (self, to),
            (Self::Created, Self::Running)
                | (Self::Running, Self::Paused)
                | (Self::Running, Self::Completed)
                | (Self::Running, Self::Failed)
                | (Self::Running, Self::Cancelled)
                | (Self::Running, Self::RollingBack)
                | (Self::Paused, Self::Running)
                | (Self::Paused, Self::Cancelled)
                | (Self::Failed, Self::RollingBack)
                | (Self::RollingBack, Self::RolledBack)
                | (Self::RollingBack, Self::Failed)
        )
    }
}

impl std::fmt::Display for WorkflowState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Running => write!(f, "running"),
            Self::Paused => write!(f, "paused"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::RollingBack => write!(f, "rolling_back"),
            Self::RolledBack => write!(f, "rolled_back"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states() {
        assert!(WorkflowState::Completed.is_terminal());
        assert!(WorkflowState::Failed.is_terminal());
        assert!(WorkflowState::RolledBack.is_terminal());
        assert!(WorkflowState::Cancelled.is_terminal());
        assert!(!WorkflowState::Running.is_terminal());
        assert!(!WorkflowState::RollingBack.is_terminal());
    }

    #[test]
    fn valid_transitions() {
        assert!(WorkflowState::Created.valid_transition(&WorkflowState::Running));
        assert!(WorkflowState::Running.valid_transition(&WorkflowState::Completed));
        assert!(WorkflowState::Running.valid_transition(&WorkflowState::RollingBack));
        assert!(WorkflowState::Failed.valid_transition(&WorkflowState::RollingBack));
        assert!(!WorkflowState::Completed.valid_transition(&WorkflowState::Running));
        assert!(!WorkflowState::Created.valid_transition(&WorkflowState::Completed));
    }

    #[test]
    fn display() {
        assert_eq!(WorkflowState::RollingBack.to_string(), "rolling_back");
    }

    #[test]
    fn no_transition_from_terminal() {
        let terminals = [
            WorkflowState::Completed,
            WorkflowState::RolledBack,
            WorkflowState::Cancelled,
        ];
        let all = [
            WorkflowState::Created,
            WorkflowState::Running,
            WorkflowState::Paused,
            WorkflowState::Completed,
            WorkflowState::Failed,
            WorkflowState::RollingBack,
            WorkflowState::RolledBack,
            WorkflowState::Cancelled,
        ];
        for t in &terminals {
            for target in &all {
                assert!(
                    !t.valid_transition(target),
                    "{t} should not transition to {target}"
                );
            }
        }
    }

    #[test]
    fn serde_roundtrip() {
        let state = WorkflowState::RollingBack;
        let json = serde_json::to_string(&state).unwrap();
        let back: WorkflowState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, state);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    fn arb_state() -> impl Strategy<Value = WorkflowState> {
        prop_oneof![
            Just(WorkflowState::Created),
            Just(WorkflowState::Running),
            Just(WorkflowState::Paused),
            Just(WorkflowState::Completed),
            Just(WorkflowState::Failed),
            Just(WorkflowState::RollingBack),
            Just(WorkflowState::RolledBack),
            Just(WorkflowState::Cancelled),
        ]
    }

    proptest! {
        /// True terminal states (no outgoing transitions) must reject all targets.
        /// Note: Failed is NOT fully terminal — it can transition to RollingBack.
        #[test]
        fn fully_terminal_states_have_no_outgoing(from in arb_state(), to in arb_state()) {
            let fully_terminal = matches!(
                from,
                WorkflowState::Completed | WorkflowState::RolledBack | WorkflowState::Cancelled
            );
            if fully_terminal {
                prop_assert!(!from.valid_transition(&to));
            }
        }

        #[test]
        fn display_never_empty(state in arb_state()) {
            prop_assert!(!state.to_string().is_empty());
        }

        #[test]
        fn serde_roundtrip_any(state in arb_state()) {
            let json = serde_json::to_string(&state).unwrap();
            let back: WorkflowState = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(back, state);
        }
    }
}
