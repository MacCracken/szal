//! Hardware-aware scheduling support.
//!
//! Requires the `hardware` feature and the `ai-hwaccel` crate.

use std::sync::Arc;

use ai_hwaccel::{AcceleratorFamily, AcceleratorRegistry, AcceleratorRequirement, CachedRegistry};

use crate::SzalError;
use crate::step::StepDef;

/// Default cache TTL for hardware detection (5 minutes).
const DEFAULT_CACHE_TTL_SECS: u64 = 300;

/// Hardware context for the execution engine.
///
/// Wraps a [`CachedRegistry`] so hardware detection results are reused
/// across flow executions without re-probing nvidia-smi, etc.
#[derive(Clone)]
pub struct HardwareContext {
    registry: Arc<CachedRegistry>,
}

impl HardwareContext {
    /// Create a hardware context with default cache TTL (5 minutes).
    #[must_use]
    pub fn detect() -> Self {
        Self::with_ttl(std::time::Duration::from_secs(DEFAULT_CACHE_TTL_SECS))
    }

    /// Create a hardware context with a custom cache TTL.
    #[must_use]
    pub fn with_ttl(ttl: std::time::Duration) -> Self {
        Self {
            registry: Arc::new(CachedRegistry::new(ttl)),
        }
    }

    /// Get the underlying cached registry snapshot.
    #[must_use]
    pub fn registry(&self) -> Arc<AcceleratorRegistry> {
        self.registry.get()
    }

    /// Validate that all steps with hardware requirements can be satisfied.
    ///
    /// Returns `Err(SzalError::HardwareUnavailable)` for the first step
    /// whose requirement cannot be met by any available device.
    pub fn check_requirements(&self, steps: &[StepDef]) -> crate::Result<()> {
        let reg = self.registry.get();
        for step in steps {
            if step.hardware == AcceleratorRequirement::None {
                continue;
            }
            let matching_count = reg.satisfying(&step.hardware).count();
            if matching_count == 0 {
                return Err(SzalError::HardwareUnavailable {
                    step: step.name.clone(),
                    requirement: format!("{:?}", step.hardware),
                });
            }
        }
        Ok(())
    }

    /// Compute effective concurrency based on hardware constraints.
    ///
    /// If steps require accelerators, concurrency is capped by the number
    /// of available devices in the required family. CPU-only steps are
    /// unconstrained.
    #[must_use]
    pub fn effective_concurrency(&self, steps: &[StepDef], base_concurrency: usize) -> usize {
        let reg = self.registry.get();

        let gpu_needed = steps.iter().any(|s| {
            matches!(
                s.hardware,
                AcceleratorRequirement::Gpu | AcceleratorRequirement::GpuOrTpu
            )
        });
        let tpu_needed = steps.iter().any(|s| {
            matches!(
                s.hardware,
                AcceleratorRequirement::Tpu { .. } | AcceleratorRequirement::GpuOrTpu
            )
        });

        let mut limit = base_concurrency;

        if gpu_needed {
            let gpu_count = reg.by_family(AcceleratorFamily::Gpu).count().max(1);
            limit = limit.min(gpu_count);
        }
        if tpu_needed {
            let tpu_count = reg.by_family(AcceleratorFamily::Tpu).count().max(1);
            limit = limit.min(tpu_count);
        }

        limit.max(1)
    }
}

impl std::fmt::Debug for HardwareContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reg = self.registry.get();
        f.debug_struct("HardwareContext")
            .field("devices", &reg.all_profiles().len())
            .field("has_accelerator", &reg.has_accelerator())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_requirements_none_always_passes() {
        let ctx = HardwareContext::detect();
        let steps = vec![StepDef::new("cpu-work")];
        assert!(ctx.check_requirements(&steps).is_ok());
    }

    #[test]
    fn check_requirements_gpu_fails_without_gpu() {
        let ctx = HardwareContext::detect();
        let reg = ctx.registry();
        // Only run this assertion if there's no GPU on the test machine
        if !reg.has_accelerator() {
            let steps = vec![StepDef::new("train").with_hardware(AcceleratorRequirement::Gpu)];
            let result = ctx.check_requirements(&steps);
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("hardware unavailable"));
            assert!(err.contains("train"));
        }
    }

    #[test]
    fn effective_concurrency_unconstrained_for_cpu_steps() {
        let ctx = HardwareContext::detect();
        let steps = vec![StepDef::new("a"), StepDef::new("b"), StepDef::new("c")];
        // CPU-only steps should not reduce concurrency
        assert_eq!(ctx.effective_concurrency(&steps, 16), 16);
    }

    #[test]
    fn effective_concurrency_caps_at_device_count() {
        let ctx = HardwareContext::detect();
        let reg = ctx.registry();
        let gpu_count = reg.by_family(AcceleratorFamily::Gpu).count();

        if gpu_count > 0 && gpu_count < 16 {
            let steps = vec![
                StepDef::new("train-1").with_hardware(AcceleratorRequirement::Gpu),
                StepDef::new("train-2").with_hardware(AcceleratorRequirement::Gpu),
            ];
            let eff = ctx.effective_concurrency(&steps, 16);
            assert!(eff <= gpu_count);
        }
    }

    #[test]
    fn hardware_context_debug() {
        let ctx = HardwareContext::detect();
        let debug = format!("{ctx:?}");
        assert!(debug.contains("HardwareContext"));
        assert!(debug.contains("devices"));
    }
}
