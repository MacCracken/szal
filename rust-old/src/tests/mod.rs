use crate::*;

#[test]
fn full_sequential_flow() {
    let mut flow = flow::FlowDef::new("deploy-pipeline", flow::FlowMode::Sequential);
    flow.add_step(step::StepDef::new("build").with_timeout(60_000));
    flow.add_step(step::StepDef::new("test").with_retries(2, 1000));
    flow.add_step(step::StepDef::new("deploy").with_rollback());
    assert_eq!(flow.steps.len(), 3);
    assert!(flow.validate().is_ok());
}

#[test]
fn dag_flow_with_diamond() {
    let build = step::StepDef::new("build");
    let test_unit = step::StepDef::new("unit-test").depends_on(build.id);
    let test_integ = step::StepDef::new("integ-test").depends_on(build.id);
    let deploy = step::StepDef::new("deploy")
        .depends_on(test_unit.id)
        .depends_on(test_integ.id);

    let mut flow = flow::FlowDef::new("diamond", flow::FlowMode::Dag);
    flow.add_step(build);
    flow.add_step(test_unit);
    flow.add_step(test_integ);
    flow.add_step(deploy);
    assert!(flow.validate().is_ok());
}

#[test]
fn workflow_state_lifecycle() {
    let s = state::WorkflowState::Created;
    assert!(s.valid_transition(&state::WorkflowState::Running));
    assert!(!s.is_terminal());
}

#[test]
fn error_display() {
    let err = SzalError::RetryExhausted {
        step: "deploy".into(),
        attempts: 3,
    };
    assert!(err.to_string().contains("deploy"));
    assert!(err.to_string().contains("3"));
}
