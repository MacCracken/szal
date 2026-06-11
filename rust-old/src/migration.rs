//! Flow versioning and migration.
//!
//! Flow definitions carry a [`version`](crate::flow::FlowDef::version). As a flow's
//! schema evolves — steps renamed, config reshaped, modes changed — older
//! persisted definitions can be brought forward by registering one
//! [`FlowMigration`] per version step and running them through a
//! [`MigrationRegistry`].
//!
//! Each migration upgrades a flow from version `N` to a strictly greater version.
//! The registry chains them, applying each in turn until the flow reaches the
//! requested target (or the latest registered version).
//!
//! ## Example
//!
//! ```
//! use szal::flow::{FlowDef, FlowMode};
//! use szal::migration::{MigrationRegistry, fn_migration};
//!
//! // v1 flows used a step named "deploy"; v2 renames it to "release".
//! let registry = MigrationRegistry::new().with_migration(fn_migration(1, 2, |mut flow| {
//!     for step in &mut flow.steps {
//!         if step.name == "deploy" {
//!             step.name = "release".into();
//!         }
//!     }
//!     Ok(flow)
//! }));
//!
//! let mut old = FlowDef::new("pipeline", FlowMode::Sequential); // version 1
//! old.add_step(szal::step::StepDef::new("deploy"));
//!
//! let upgraded = registry.migrate_latest(old).unwrap();
//! assert_eq!(upgraded.version, 2);
//! assert_eq!(upgraded.steps[0].name, "release");
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use crate::SzalError;
use crate::flow::FlowDef;

/// A single-step upgrade of a [`FlowDef`] from one version to a greater one.
///
/// Implementations transform the flow's contents; the [`MigrationRegistry`]
/// is responsible for chaining migrations and stamping the resulting
/// [`version`](crate::flow::FlowDef::version).
pub trait FlowMigration: Send + Sync {
    /// The version this migration upgrades *from*.
    fn source_version(&self) -> u32;

    /// The version this migration upgrades *to*. Must be greater than
    /// [`source_version`](Self::source_version).
    fn target_version(&self) -> u32;

    /// Transform a flow at [`source_version`](Self::source_version) into one whose
    /// contents match [`target_version`](Self::target_version).
    ///
    /// The returned flow's `version` field is overwritten by the registry, so
    /// implementations need not set it.
    fn migrate(&self, flow: FlowDef) -> Result<FlowDef, SzalError>;
}

/// Build a [`FlowMigration`] from a closure.
///
/// Convenient when a migration is a simple transform and a dedicated type is
/// overkill.
#[must_use]
pub fn fn_migration<F>(from: u32, to: u32, f: F) -> Arc<dyn FlowMigration>
where
    F: Fn(FlowDef) -> Result<FlowDef, SzalError> + Send + Sync + 'static,
{
    Arc::new(FnMigration { from, to, f })
}

struct FnMigration<F> {
    from: u32,
    to: u32,
    f: F,
}

impl<F> FlowMigration for FnMigration<F>
where
    F: Fn(FlowDef) -> Result<FlowDef, SzalError> + Send + Sync,
{
    fn source_version(&self) -> u32 {
        self.from
    }
    fn target_version(&self) -> u32 {
        self.to
    }
    fn migrate(&self, flow: FlowDef) -> Result<FlowDef, SzalError> {
        (self.f)(flow)
    }
}

/// A registry of [`FlowMigration`]s, chained to upgrade flows across versions.
///
/// At most one migration may be registered per source version. Migrations are
/// applied in ascending order, each advancing the flow's version, until the
/// target is reached.
#[derive(Default, Clone)]
pub struct MigrationRegistry {
    /// `source_version` → migration.
    migrations: HashMap<u32, Arc<dyn FlowMigration>>,
}

impl MigrationRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a migration, returning `self` for chaining.
    ///
    /// # Panics
    ///
    /// Panics if `target_version <= source_version`, or if a migration is already
    /// registered for the same source version. Migrations are part of program
    /// definition, so a malformed set is a programmer error caught at setup.
    #[must_use]
    pub fn with_migration(mut self, migration: Arc<dyn FlowMigration>) -> Self {
        self.register(migration);
        self
    }

    /// Register a migration in place.
    ///
    /// # Panics
    ///
    /// See [`with_migration`](Self::with_migration).
    pub fn register(&mut self, migration: Arc<dyn FlowMigration>) {
        let from = migration.source_version();
        let to = migration.target_version();
        assert!(
            to > from,
            "migration target_version ({to}) must be greater than source_version ({from})"
        );
        assert!(
            !self.migrations.contains_key(&from),
            "a migration is already registered for version {from}"
        );
        self.migrations.insert(from, migration);
    }

    /// The highest target version reachable by registered migrations, or `None`
    /// if the registry is empty.
    #[must_use]
    pub fn latest_version(&self) -> Option<u32> {
        self.migrations.values().map(|m| m.target_version()).max()
    }

    /// Migrate `flow` up to `target`, applying registered migrations in order.
    ///
    /// Returns the flow unchanged if it is already at `target`. Errors if the
    /// flow is newer than `target` (no downgrades), or if no migration path
    /// exists from the current version to `target`.
    pub fn migrate_to(&self, mut flow: FlowDef, target: u32) -> Result<FlowDef, SzalError> {
        if flow.version == target {
            return Ok(flow);
        }
        if flow.version > target {
            return Err(SzalError::MigrationFailed(format!(
                "flow '{}' is at version {} which is newer than target {target}; downgrades are not supported",
                flow.name, flow.version
            )));
        }

        while flow.version < target {
            let current = flow.version;
            let migration = self.migrations.get(&current).ok_or_else(|| {
                SzalError::MigrationFailed(format!(
                    "no migration registered from version {current} (flow '{}', target {target})",
                    flow.name
                ))
            })?;
            let to = migration.target_version();
            if to > target {
                return Err(SzalError::MigrationFailed(format!(
                    "migration from version {current} jumps to {to}, overshooting target {target} (flow '{}')",
                    flow.name
                )));
            }
            let name = flow.name.clone();
            flow = migration.migrate(flow)?;
            flow.version = to;
            tracing::debug!(flow = %name, from = current, to, "applied flow migration");
        }

        Ok(flow)
    }

    /// Migrate `flow` up to the latest registered version.
    ///
    /// A no-op if the registry is empty or the flow is already current.
    pub fn migrate_latest(&self, flow: FlowDef) -> Result<FlowDef, SzalError> {
        match self.latest_version() {
            Some(target) if target > flow.version => self.migrate_to(flow, target),
            _ => Ok(flow),
        }
    }
}

impl std::fmt::Debug for MigrationRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut versions: Vec<_> = self
            .migrations
            .values()
            .map(|m| (m.source_version(), m.target_version()))
            .collect();
        versions.sort_unstable();
        f.debug_struct("MigrationRegistry")
            .field("migrations", &versions)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::{FlowDef, FlowMode};
    use crate::step::StepDef;

    fn flow_v(version: u32) -> FlowDef {
        let mut flow = FlowDef::new("pipeline", FlowMode::Sequential).with_version(version);
        flow.add_step(StepDef::new("deploy"));
        flow
    }

    #[test]
    fn migrate_single_step() {
        let registry = MigrationRegistry::new().with_migration(fn_migration(1, 2, |mut flow| {
            flow.steps[0].name = "release".into();
            Ok(flow)
        }));
        let out = registry.migrate_to(flow_v(1), 2).unwrap();
        assert_eq!(out.version, 2);
        assert_eq!(out.steps[0].name, "release");
    }

    #[test]
    fn migrate_chains_multiple_versions() {
        let registry = MigrationRegistry::new()
            .with_migration(fn_migration(1, 2, |mut flow| {
                flow.description = "v2".into();
                Ok(flow)
            }))
            .with_migration(fn_migration(2, 3, |mut flow| {
                flow.add_step(StepDef::new("verify"));
                Ok(flow)
            }));
        let out = registry.migrate_latest(flow_v(1)).unwrap();
        assert_eq!(out.version, 3);
        assert_eq!(out.description, "v2");
        assert_eq!(out.steps.len(), 2);
        assert_eq!(out.steps[1].name, "verify");
    }

    #[test]
    fn already_at_target_is_noop() {
        let registry = MigrationRegistry::new().with_migration(fn_migration(1, 2, Ok));
        let out = registry.migrate_to(flow_v(2), 2).unwrap();
        assert_eq!(out.version, 2);
    }

    #[test]
    fn empty_registry_migrate_latest_noop() {
        let registry = MigrationRegistry::new();
        let out = registry.migrate_latest(flow_v(1)).unwrap();
        assert_eq!(out.version, 1);
        assert!(registry.latest_version().is_none());
    }

    #[test]
    fn downgrade_is_rejected() {
        let registry = MigrationRegistry::new().with_migration(fn_migration(1, 2, Ok));
        let err = registry.migrate_to(flow_v(3), 2).unwrap_err();
        assert!(matches!(err, SzalError::MigrationFailed(_)));
    }

    #[test]
    fn missing_migration_path_errors() {
        // Registry can go 1→2 but not 2→3; targeting 3 from 1 fails at version 2.
        let registry = MigrationRegistry::new().with_migration(fn_migration(1, 2, Ok));
        let err = registry.migrate_to(flow_v(1), 3).unwrap_err();
        match err {
            SzalError::MigrationFailed(m) => assert!(m.contains("no migration registered")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn overshoot_target_errors() {
        // 1→3 migration but target is 2: must not overshoot.
        let registry = MigrationRegistry::new().with_migration(fn_migration(1, 3, Ok));
        let err = registry.migrate_to(flow_v(1), 2).unwrap_err();
        assert!(matches!(err, SzalError::MigrationFailed(_)));
    }

    #[test]
    fn migration_error_propagates() {
        let registry = MigrationRegistry::new().with_migration(fn_migration(1, 2, |_flow| {
            Err(SzalError::MigrationFailed("boom".into()))
        }));
        assert!(registry.migrate_to(flow_v(1), 2).is_err());
    }

    #[test]
    #[should_panic(expected = "already registered")]
    fn duplicate_source_version_panics() {
        let _ = MigrationRegistry::new()
            .with_migration(fn_migration(1, 2, Ok))
            .with_migration(fn_migration(1, 3, Ok));
    }

    #[test]
    #[should_panic(expected = "must be greater")]
    fn non_increasing_version_panics() {
        let _ = MigrationRegistry::new().with_migration(fn_migration(2, 2, Ok));
    }
}
