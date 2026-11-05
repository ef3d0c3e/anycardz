//! Attempt limits: a token bucket per key (an account, a client address). Each
//! counted attempt takes a token; tokens come back one every `refill`, up to `burst`.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use actix_web::HttpRequest;

/// Buckets kept before full ones are forgotten: memory stays bounded
const MAX_KEYS: usize = 10_000;

pub struct Limiter {
    burst: f64,
    refill: Duration,
    buckets: Mutex<HashMap<String, (f64, Instant)>>,
}

impl Limiter {
    pub fn new(burst: u32, refill: Duration) -> Self {
        Limiter { burst: burst as f64, refill, buckets: Mutex::new(HashMap::new()) }
    }

    fn level(&self, tokens: f64, last: Instant, now: Instant) -> f64 {
        (tokens + now.duration_since(last).as_secs_f64() / self.refill.as_secs_f64()).min(self.burst)
    }

    /// How long until `key` may try again, None when it may now
    pub fn wait_at(&self, key: &str, now: Instant) -> Option<Duration> {
        let buckets = self.buckets.lock().unwrap();
        let tokens = buckets.get(key).map_or(self.burst, |&(t, last)| self.level(t, last, now));
        (tokens < 1.0).then(|| self.refill.mul_f64(1.0 - tokens))
    }

    /// Counts one attempt for `key`
    pub fn hit_at(&self, key: &str, now: Instant) {
        let mut buckets = self.buckets.lock().unwrap();
        if buckets.len() >= MAX_KEYS {
            buckets.retain(|_, &mut (t, last)| self.level(t, last, now) < self.burst);
        }
        let tokens = buckets.get(key).map_or(self.burst, |&(t, last)| self.level(t, last, now));
        buckets.insert(key.to_owned(), ((tokens - 1.0).max(0.0), now));
    }

    pub fn wait(&self, key: &str) -> Option<Duration> {
        self.wait_at(key, Instant::now())
    }

    pub fn hit(&self, key: &str) {
        self.hit_at(key, Instant::now())
    }
}

/// The limits of the auth endpoints
pub struct AuthLimits {
    /// Failed logins per account: 5, then one a minute (as in the Go version)
    pub login_account: Limiter,
    /// Failed logins per client address: one address cannot sweep many accounts
    pub login_ip: Limiter,
    /// Sign-ups per client address: each new account comes with coins
    pub register_ip: Limiter,
}

impl AuthLimits {
    /// For development and tests, which sign up many players from one address
    pub fn relaxed() -> Self {
        AuthLimits {
            login_account: Limiter::new(1000, Duration::from_millis(1)),
            login_ip: Limiter::new(100_000, Duration::from_millis(1)),
            register_ip: Limiter::new(100_000, Duration::from_millis(1)),
        }
    }
}

impl Default for AuthLimits {
    fn default() -> Self {
        AuthLimits {
            login_account: Limiter::new(5, Duration::from_secs(60)),
            login_ip: Limiter::new(20, Duration::from_secs(15)),
            register_ip: Limiter::new(5, Duration::from_secs(600)),
        }
    }
}

/// The client's address. `X-Forwarded-For` counts only when the request comes
/// from a trusted proxy, else anyone could pick their own address.
pub fn client_ip(req: &HttpRequest, trusted: &[IpAddr]) -> String {
    let peer = req.peer_addr().map(|a| a.ip());
    let Some(peer) = peer else { return "unknown".into() };
    if !trusted.contains(&peer) {
        return peer.to_string();
    }
    // The rightmost address no trusted proxy added is the client's
    let forwarded = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()).unwrap_or("");
    forwarded
        .split(',')
        .filter_map(|s| s.trim().parse::<IpAddr>().ok())
        .rev()
        .find(|ip| !trusted.contains(ip))
        .unwrap_or(peer)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    #[test]
    fn burst_then_refill() {
        let l = Limiter::new(3, Duration::from_secs(60));
        let t0 = Instant::now();
        for _ in 0..3 {
            assert_eq!(l.wait_at("a", t0), None);
            l.hit_at("a", t0);
        }
        let wait = l.wait_at("a", t0).expect("out of tokens");
        assert_eq!(wait, Duration::from_secs(60));
        assert_eq!(l.wait_at("b", t0), None, "keys are separate");
        // Half a refill later: half a token, still waiting 30 s
        assert_eq!(l.wait_at("a", t0 + Duration::from_secs(30)), Some(Duration::from_secs(30)));
        assert_eq!(l.wait_at("a", t0 + Duration::from_secs(60)), None);
        // Never more than the burst
        assert_eq!(l.wait_at("a", t0 + Duration::from_secs(3600)), None);
        for _ in 0..3 {
            l.hit_at("a", t0 + Duration::from_secs(3600));
        }
        assert!(l.wait_at("a", t0 + Duration::from_secs(3600)).is_some());
    }

    #[test]
    fn forwarded_for_only_from_trusted_proxies() {
        let proxy: IpAddr = "10.0.0.2".parse().unwrap();
        let req = TestRequest::default()
            .peer_addr("10.0.0.2:5000".parse().unwrap())
            .insert_header(("x-forwarded-for", "6.6.6.6, 203.0.113.9"))
            .to_http_request();
        assert_eq!(client_ip(&req, &[proxy]), "203.0.113.9", "rightmost untrusted address");
        assert_eq!(client_ip(&req, &[]), "10.0.0.2", "untrusted peer: header ignored");
        let direct = TestRequest::default()
            .peer_addr("198.51.100.4:5000".parse().unwrap())
            .insert_header(("x-forwarded-for", "1.2.3.4"))
            .to_http_request();
        assert_eq!(client_ip(&direct, &[proxy]), "198.51.100.4", "a client cannot pick its address");
    }
}
