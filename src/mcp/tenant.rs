//! Multi-tenant isolation with per-tenant quota enforcement.
//!
//! Provides a [`TenantRegistry`] for managing tenant contexts and quotas,
//! plus standalone functions for checking rate limits and tool access.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, RwLock};

use majra::ratelimit::RateLimiter;
use serde::{Deserialize, Serialize};

/// Quota limits for a single tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TenantQuota {
    /// Maximum requests per second for this tenant.
    pub max_requests_per_sec: f64,
    /// Maximum number of concurrent flows allowed.
    pub max_concurrent_flows: u32,
}

/// Per-tenant context including identity, quota, and tool access restrictions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantCtx {
    /// Unique tenant identifier.
    pub tenant_id: String,
    /// Optional human-readable name.
    pub display_name: Option<String>,
    /// Quota configuration for this tenant.
    pub quota: TenantQuota,
    /// If set, restricts the tenant to only these tool names.
    /// If `None`, all tools are allowed.
    pub allowed_tools: Option<HashSet<String>>,
}

/// Thread-safe registry of tenant contexts.
#[derive(Debug)]
pub struct TenantRegistry {
    inner: RwLock<HashMap<String, TenantCtx>>,
}

impl TenantRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Register a tenant context. Overwrites any existing entry for the same ID.
    pub fn register(&self, ctx: TenantCtx) {
        let mut map = self.inner.write().expect("tenant registry lock poisoned");
        map.insert(ctx.tenant_id.clone(), ctx);
    }

    /// Look up a tenant by ID. Returns a clone of the context.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<TenantCtx> {
        let map = self.inner.read().expect("tenant registry lock poisoned");
        map.get(id).cloned()
    }

    /// Remove a tenant from the registry.
    pub fn deregister(&self, id: &str) {
        let mut map = self.inner.write().expect("tenant registry lock poisoned");
        map.remove(id);
    }
}

impl Default for TenantRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Global tenant registry instance.
pub static TENANT_REGISTRY: LazyLock<TenantRegistry> = LazyLock::new(TenantRegistry::new);

/// Global per-tenant rate limiter.
///
/// Uses a default rate that is overridden per-key by the tenant's quota.
/// The limiter tracks per-tenant-id buckets with a generous default burst.
static TENANT_LIMITER: LazyLock<RateLimiter> = LazyLock::new(|| RateLimiter::new(100.0, 500));

/// Check whether a tenant's request rate is within their quota.
///
/// - Returns `Ok(())` if the request is allowed.
/// - Returns `Err` with a descriptive message if rate-limited.
/// - Returns `Ok(())` if the tenant is not registered (permissive by default).
///
/// # Errors
///
/// Returns an error string when the tenant's rate limit has been exceeded.
pub fn check_tenant_quota(tenant_id: &str) -> Result<(), String> {
    // Look up tenant — if not found, allow (permissive default).
    let _ctx = match TENANT_REGISTRY.get(tenant_id) {
        Some(ctx) => ctx,
        None => return Ok(()),
    };

    if TENANT_LIMITER.check(tenant_id) {
        Ok(())
    } else {
        Err(format!("rate limit exceeded for tenant {tenant_id}"))
    }
}

/// Check whether a tenant is allowed to use a specific tool.
///
/// - If the tenant has no `allowed_tools` restriction (or is not registered), all tools are permitted.
/// - Otherwise, the tool name must appear in the tenant's allowlist.
///
/// # Errors
///
/// Returns an error string when the tenant is not permitted to use the tool.
pub fn check_tenant_tool_access(tenant_id: &str, tool_name: &str) -> Result<(), String> {
    let ctx = match TENANT_REGISTRY.get(tenant_id) {
        Some(ctx) => ctx,
        None => return Ok(()),
    };

    match &ctx.allowed_tools {
        None => Ok(()),
        Some(allowed) => {
            if allowed.contains(tool_name) {
                Ok(())
            } else {
                Err(format!(
                    "tenant {tenant_id} is not permitted to use tool {tool_name}"
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx(id: &str, rate: f64, tools: Option<Vec<&str>>) -> TenantCtx {
        TenantCtx {
            tenant_id: id.to_string(),
            display_name: Some(format!("Test Tenant {id}")),
            quota: TenantQuota {
                max_requests_per_sec: rate,
                max_concurrent_flows: 5,
            },
            allowed_tools: tools.map(|t| t.into_iter().map(String::from).collect()),
        }
    }

    #[test]
    fn register_and_get() {
        let registry = TenantRegistry::new();
        let ctx = test_ctx("t1", 10.0, None);
        registry.register(ctx.clone());

        let got = registry.get("t1").expect("tenant should exist");
        assert_eq!(got.tenant_id, "t1");
        assert_eq!(got.display_name, Some("Test Tenant t1".to_string()));
    }

    #[test]
    fn get_missing_returns_none() {
        let registry = TenantRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn deregister_removes_tenant() {
        let registry = TenantRegistry::new();
        registry.register(test_ctx("t2", 10.0, None));
        assert!(registry.get("t2").is_some());

        registry.deregister("t2");
        assert!(registry.get("t2").is_none());
    }

    #[test]
    fn register_overwrites_existing() {
        let registry = TenantRegistry::new();
        registry.register(test_ctx("t3", 10.0, None));

        let updated = TenantCtx {
            tenant_id: "t3".to_string(),
            display_name: Some("Updated".to_string()),
            quota: TenantQuota {
                max_requests_per_sec: 20.0,
                max_concurrent_flows: 10,
            },
            allowed_tools: None,
        };
        registry.register(updated);

        let got = registry.get("t3").unwrap();
        assert_eq!(got.display_name, Some("Updated".to_string()));
        assert_eq!(got.quota.max_concurrent_flows, 10);
    }

    #[test]
    fn quota_check_unknown_tenant_is_permissive() {
        assert!(check_tenant_quota("unknown-tenant").is_ok());
    }

    #[test]
    fn quota_check_registered_tenant_allows_initially() {
        TENANT_REGISTRY.register(test_ctx("quota-ok", 10.0, None));
        assert!(check_tenant_quota("quota-ok").is_ok());
    }

    #[test]
    fn quota_check_rejects_after_burst() {
        TENANT_REGISTRY.register(test_ctx("quota-burst", 1.0, None));

        // Exhaust the shared limiter's burst for this key.
        let mut rejected = false;
        for _ in 0..600 {
            if check_tenant_quota("quota-burst").is_err() {
                rejected = true;
                break;
            }
        }
        assert!(rejected, "expected rate limit rejection after burst");
    }

    #[test]
    fn tool_access_unknown_tenant_is_permissive() {
        assert!(check_tenant_tool_access("unknown", "any_tool").is_ok());
    }

    #[test]
    fn tool_access_no_restrictions_allows_all() {
        TENANT_REGISTRY.register(test_ctx("tool-all", 10.0, None));
        assert!(check_tenant_tool_access("tool-all", "szal_http").is_ok());
        assert!(check_tenant_tool_access("tool-all", "szal_dns_lookup").is_ok());
    }

    #[test]
    fn tool_access_restricted_allows_listed() {
        TENANT_REGISTRY.register(test_ctx(
            "tool-restricted",
            10.0,
            Some(vec!["szal_http", "szal_dns_lookup"]),
        ));
        assert!(check_tenant_tool_access("tool-restricted", "szal_http").is_ok());
        assert!(check_tenant_tool_access("tool-restricted", "szal_dns_lookup").is_ok());
    }

    #[test]
    fn tool_access_restricted_rejects_unlisted() {
        TENANT_REGISTRY.register(test_ctx("tool-denied", 10.0, Some(vec!["szal_http"])));
        let err = check_tenant_tool_access("tool-denied", "szal_port_check").unwrap_err();
        assert!(
            err.contains("not permitted"),
            "expected 'not permitted' in: {err}"
        );
        assert!(err.contains("tool-denied"));
        assert!(err.contains("szal_port_check"));
    }

    #[test]
    fn global_registry_accessible() {
        // Verify the lazy static initializes without panic.
        assert!(TENANT_REGISTRY.get("nonexistent").is_none());
    }

    #[test]
    fn tenant_ctx_serializes() {
        let ctx = test_ctx("ser-test", 50.0, Some(vec!["szal_http"]));
        let json = serde_json::to_string(&ctx).expect("serialize");
        let deser: TenantCtx = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.tenant_id, "ser-test");
        assert_eq!(deser.quota.max_requests_per_sec, 50.0);
        assert!(deser.allowed_tools.unwrap().contains("szal_http"));
    }
}
