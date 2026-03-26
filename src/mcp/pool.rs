//! Connection pooling and backpressure for network tools.
//!
//! Provides per-category rate limiting (HTTP, DNS, port scanning) using
//! [`majra::ratelimit::RateLimiter`] to prevent abuse of network tools.

use std::sync::LazyLock;

use majra::ratelimit::RateLimiter;

/// Per-category rate limiters for network tools.
///
/// Each network tool category (HTTP, DNS, port scanning) has its own
/// [`RateLimiter`] with independent rate/burst settings. Callers use
/// the `check_*` methods before dispatching a request.
pub struct NetworkPool {
    http: RateLimiter,
    dns: RateLimiter,
    port: RateLimiter,
}

impl NetworkPool {
    /// Create a new pool with default rate limits.
    ///
    /// Defaults:
    /// - HTTP: 10 req/s, burst 50
    /// - DNS: 100 req/s, burst 200
    /// - Port: 50 req/s, burst 100
    #[must_use]
    pub fn new() -> Self {
        Self {
            http: RateLimiter::new(10.0, 50),
            dns: RateLimiter::new(100.0, 200),
            port: RateLimiter::new(50.0, 100),
        }
    }

    /// Check if an HTTP request to `host` is allowed under the rate limit.
    #[inline]
    pub fn check_http(&self, host: &str) -> bool {
        self.http.check(host)
    }

    /// Check if a DNS lookup for `domain` is allowed under the rate limit.
    #[inline]
    pub fn check_dns(&self, domain: &str) -> bool {
        self.dns.check(domain)
    }

    /// Check if a port scan for `key` is allowed under the rate limit.
    #[inline]
    pub fn check_port(&self, key: &str) -> bool {
        self.port.check(key)
    }
}

impl Default for NetworkPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Global network pool instance.
pub static NETWORK_POOL: LazyLock<NetworkPool> = LazyLock::new(NetworkPool::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_rate_limit_exhausts_burst() {
        let pool = NetworkPool::new();
        // HTTP burst is 50 — exhaust all tokens for a single host.
        let mut allowed = 0;
        for _ in 0..60 {
            if pool.check_http("example.com") {
                allowed += 1;
            }
        }
        assert_eq!(allowed, 50, "expected exactly 50 allowed from burst");
    }

    #[test]
    fn http_rate_limit_eventually_rejects() {
        let pool = NetworkPool::new();
        // Drain the burst for one host.
        for _ in 0..50 {
            pool.check_http("test.com");
        }
        // Next request should be rejected.
        assert!(
            !pool.check_http("test.com"),
            "expected rejection after burst exhausted"
        );
    }

    #[test]
    fn dns_rate_limit_exhausts_burst() {
        let pool = NetworkPool::new();
        let mut allowed = 0;
        for _ in 0..210 {
            if pool.check_dns("example.com") {
                allowed += 1;
            }
        }
        assert_eq!(allowed, 200, "expected exactly 200 allowed from DNS burst");
    }

    #[test]
    fn port_rate_limit_exhausts_burst() {
        let pool = NetworkPool::new();
        let mut allowed = 0;
        for _ in 0..110 {
            if pool.check_port("host:8080") {
                allowed += 1;
            }
        }
        assert_eq!(allowed, 100, "expected exactly 100 allowed from port burst");
    }

    #[test]
    fn separate_hosts_have_independent_limits() {
        let pool = NetworkPool::new();
        // Exhaust host A.
        for _ in 0..50 {
            pool.check_http("host-a.com");
        }
        assert!(!pool.check_http("host-a.com"));
        // Host B should still have full burst.
        assert!(pool.check_http("host-b.com"));
    }

    #[test]
    fn global_pool_accessible() {
        // Just verify the lazy static initializes without panic.
        assert!(NETWORK_POOL.check_dns("global-test.com"));
    }

    #[test]
    fn refills_after_time() {
        let pool = NetworkPool::new();
        // Exhaust HTTP burst.
        for _ in 0..50 {
            pool.check_http("refill.com");
        }
        assert!(!pool.check_http("refill.com"));

        // Wait for refill (10 req/s = 1 token per 100ms).
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(
            pool.check_http("refill.com"),
            "expected token refill after waiting"
        );
    }
}
