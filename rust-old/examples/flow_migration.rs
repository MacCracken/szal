//! Flow versioning and migration: bring an old flow definition forward.

use szal::flow::{FlowDef, FlowMode};
use szal::migration::{MigrationRegistry, fn_migration};
use szal::step::StepDef;

fn main() {
    // A registry describing how the "deploy-pipeline" schema evolved:
    //   v1 → v2: the "deploy" step was renamed to "release"
    //   v2 → v3: a "smoke-test" step was appended
    let registry = MigrationRegistry::new()
        .with_migration(fn_migration(1, 2, |mut flow| {
            for step in &mut flow.steps {
                if step.name == "deploy" {
                    step.name = "release".into();
                }
            }
            Ok(flow)
        }))
        .with_migration(fn_migration(2, 3, |mut flow| {
            flow.add_step(StepDef::new("smoke-test"));
            Ok(flow)
        }));

    // An old (version 1) definition loaded from storage.
    let mut old = FlowDef::new("deploy-pipeline", FlowMode::Sequential);
    old.add_step(StepDef::new("build"));
    old.add_step(StepDef::new("deploy"));

    println!(
        "before: v{} steps={:?}",
        old.version,
        old.steps.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    let upgraded = registry.migrate_latest(old).expect("migration failed");

    println!(
        "after:  v{} steps={:?}",
        upgraded.version,
        upgraded.steps.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    assert_eq!(upgraded.version, 3);
    assert_eq!(upgraded.steps[1].name, "release");
    assert_eq!(upgraded.steps[2].name, "smoke-test");
}
