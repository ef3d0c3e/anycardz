use std::path::PathBuf;

use anyhow::Context;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub bind: String,
    pub dev_collections: bool,
    pub secure_cookies: bool,
    /// Proxies whose X-Forwarded-For is believed
    pub trusted_proxies: Vec<std::net::IpAddr>,
    /// Where Prometheus scrapes /metrics; no metrics listener when unset
    pub metrics_bind: Option<String>,
    /// Bearer token Prometheus must send, when set
    pub metrics_token: Option<String>,
    /// Who runs the site and how to reach them, shown in the terms
    pub operator: Option<String>,
    pub contact: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Config> {
        let flag = |name: &str| std::env::var(name).is_ok_and(|v| v == "1" || v == "true");
        Ok(Config {
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL is not set")?,
            data_dir: std::env::var("ANYCARDZ_DATA").context("ANYCARDZ_DATA is not set")?.into(),
            cache_dir: std::env::var("ANYCARDZ_CACHE").unwrap_or_else(|_| "backend/cache".into()).into(),
            bind: std::env::var("ANYCARDZ_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into()),
            dev_collections: flag("ANYCARDZ_DEV_COLLECTIONS"),
            secure_cookies: flag("ANYCARDZ_SECURE_COOKIES"),
            trusted_proxies: std::env::var("ANYCARDZ_TRUSTED_PROXIES")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().parse().context("ANYCARDZ_TRUSTED_PROXIES holds an address that is not an IP"))
                .collect::<anyhow::Result<_>>()?,
            metrics_bind: std::env::var("ANYCARDZ_METRICS_BIND").ok().filter(|s| !s.trim().is_empty()),
            metrics_token: std::env::var("ANYCARDZ_METRICS_TOKEN").ok().filter(|s| !s.trim().is_empty()),
            operator: std::env::var("ANYCARDZ_OPERATOR").ok().filter(|s| !s.trim().is_empty()),
            contact: std::env::var("ANYCARDZ_CONTACT").ok().filter(|s| !s.trim().is_empty()),
        })
    }
}
