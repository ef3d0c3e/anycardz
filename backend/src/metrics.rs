//! Prometheus metrics. Prometheus runs elsewhere and scrapes `GET /metrics`,
//! served on its own listener (ANYCARDZ_METRICS_BIND), never through the
//! public site. A bearer token (ANYCARDZ_METRICS_TOKEN) can guard it too.
//!
//! Counters only grow while the process runs; Grafana reads them through
//! `rate()`/`increase()`, which handle restarts. Gauges (sockets, pool, users)
//! are read at scrape time.

use std::time::Instant;

use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;
use actix_web::{HttpRequest, HttpResponse, get, web};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::{Histogram, exponential_buckets};
use prometheus_client::registry::Registry;

use crate::AppState;
use crate::data::Rarity;

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct HttpLabels {
    method: String,
    /// The route pattern (`/api/trades/{id}`), never the raw path: bounded
    route: String,
    status: u16,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct RouteLabels {
    method: String,
    route: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct CollectionLabels {
    collection: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct CardLabels {
    /// pack, tradeup, roulette
    source: &'static str,
    rarity: &'static str,
    foil: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct CoinLabels {
    /// What the coins went to or came from: booster, sell, streak, topup, signup
    reason: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct ResultLabels {
    result: &'static str,
}

impl ResultLabels {
    pub fn new(result: &'static str) -> Self {
        ResultLabels { result }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct KindLabels {
    kind: &'static str,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct AuthLabels {
    /// login, register
    action: &'static str,
    /// ok, failed, limited
    result: &'static str,
}

fn rarity(r: Rarity) -> &'static str {
    match r {
        Rarity::Common => "common",
        Rarity::Rare => "rare",
        Rarity::Legendary => "legendary",
    }
}

pub struct Metrics {
    registry: Registry,
    http_requests: Family<HttpLabels, Counter>,
    http_duration: Family<RouteLabels, Histogram>,
    boosters_bought: Family<CollectionLabels, Counter>,
    boosters_opened: Family<CollectionLabels, Counter>,
    cards: Family<CardLabels, Counter>,
    coins_spent: Family<CoinLabels, Counter>,
    coins_earned: Family<CoinLabels, Counter>,
    roulette: Family<ResultLabels, Counter>,
    trades: Family<ResultLabels, Counter>,
    market: Family<ResultLabels, Counter>,
    market_fees: Counter,
    push: Family<ResultLabels, Counter>,
    auth: Family<AuthLabels, Counter>,
    shouts: Counter,
    free_packs: Counter,
    achievements: Family<KindLabels, Counter>,
    sockets: Gauge,
    online: Gauge,
    users: Gauge,
    pool_size: Gauge,
    pool_available: Gauge,
    pool_waiting: Gauge,
}

impl Default for Metrics {
    fn default() -> Self {
        let mut registry = Registry::with_prefix("anycardz");
        let http_requests = Family::<HttpLabels, Counter>::default();
        registry.register("http_requests", "HTTP requests answered", http_requests.clone());
        // 5 ms to ~10 s
        let http_duration = Family::<RouteLabels, Histogram>::new_with_constructor(|| {
            Histogram::new(exponential_buckets(0.005, 2.0, 12))
        });
        registry.register("http_request_duration_seconds", "Time to answer HTTP requests", http_duration.clone());

        let m = Metrics {
            http_requests,
            http_duration,
            boosters_bought: Default::default(),
            boosters_opened: Default::default(),
            cards: Default::default(),
            coins_spent: Default::default(),
            coins_earned: Default::default(),
            roulette: Default::default(),
            trades: Default::default(),
            market: Default::default(),
            market_fees: Default::default(),
            push: Default::default(),
            auth: Default::default(),
            shouts: Default::default(),
            free_packs: Default::default(),
            achievements: Default::default(),
            sockets: Default::default(),
            online: Default::default(),
            users: Default::default(),
            pool_size: Default::default(),
            pool_available: Default::default(),
            pool_waiting: Default::default(),
            registry,
        };
        let mut r = m.registry;
        r.register("boosters_bought", "Boosters bought, by collection", m.boosters_bought.clone());
        r.register("boosters_opened", "Boosters opened, by collection", m.boosters_opened.clone());
        r.register("cards_obtained", "Cards gained from packs and the forge", m.cards.clone());
        r.register("coins_spent", "Coins leaving players' balances", m.coins_spent.clone());
        r.register("coins_earned", "Coins entering players' balances", m.coins_earned.clone());
        r.register("roulette_spins", "Foil roulette spins, by result (won, lost)", m.roulette.clone());
        r.register("trades", "Trades ended, by result (done, cancelled)", m.trades.clone());
        r.register("market_listings", "Marketplace listings, by event (listed, withdrawn, sold)", m.market.clone());
        r.register("push_messages", "Web Push deliveries, by result (sent, gone, failed)", m.push.clone());
        r.register("market_fees_coins", "Coins taken as marketplace fees, out of the economy", m.market_fees.clone());
        r.register("auth_attempts", "Logins and sign-ups, by result", m.auth.clone());
        r.register("shouts", "Shoutbox messages posted", m.shouts.clone());
        r.register("achievements", "Achievements earned, by kind (set, foil_set, ...)", m.achievements.clone());
        r.register("free_pack_claims", "Free pack claims (each gives several packs)", m.free_packs.clone());
        r.register("live_sockets", "Open live sockets", m.sockets.clone());
        r.register("online_users", "Players with a live socket open", m.online.clone());
        r.register("users", "Registered accounts", m.users.clone());
        r.register("db_pool_connections", "Database connections in the pool", m.pool_size.clone());
        r.register("db_pool_available", "Idle database connections", m.pool_available.clone());
        r.register("db_pool_waiting", "Requests waiting for a database connection", m.pool_waiting.clone());
        Metrics { registry: r, ..m }
    }
}

impl Metrics {
    pub fn booster_bought(&self, collection: &str) {
        self.boosters_bought.get_or_create(&CollectionLabels { collection: collection.to_owned() }).inc();
    }

    pub fn booster_opened(&self, collection: &str) {
        self.boosters_opened.get_or_create(&CollectionLabels { collection: collection.to_owned() }).inc();
    }

    /// `source`: pack, tradeup, roulette
    pub fn card(&self, source: &'static str, r: Rarity, foil: bool) {
        let foil = if foil { "yes" } else { "no" };
        self.cards.get_or_create(&CardLabels { source, rarity: rarity(r), foil }).inc();
    }

    pub fn spent(&self, reason: &'static str, coins: i64) {
        self.coins_spent.get_or_create(&CoinLabels { reason }).inc_by(coins.max(0) as u64);
    }

    pub fn earned(&self, reason: &'static str, coins: i64) {
        self.coins_earned.get_or_create(&CoinLabels { reason }).inc_by(coins.max(0) as u64);
    }

    pub fn roulette(&self, won: bool) {
        self.roulette.get_or_create(&ResultLabels { result: if won { "won" } else { "lost" } }).inc();
    }

    /// `result`: done, cancelled
    pub fn market(&self, result: &'static str) {
        self.market.get_or_create(&ResultLabels { result }).inc();
    }

    /// The push counter, for deliveries that finish in the background
    pub fn push_counter(&self) -> Family<ResultLabels, Counter> {
        self.push.clone()
    }

    pub fn market_fee(&self, coins: i64) {
        self.market_fees.inc_by(coins.max(0) as u64);
    }

    pub fn trade(&self, result: &'static str) {
        self.trades.get_or_create(&ResultLabels { result }).inc();
    }

    /// `action`: login, register; `result`: ok, failed, limited
    pub fn auth(&self, action: &'static str, result: &'static str) {
        self.auth.get_or_create(&AuthLabels { action, result }).inc();
    }

    pub fn shout(&self) {
        self.shouts.inc();
    }

    pub fn achievement(&self, kind: &'static str) {
        self.achievements.get_or_create(&KindLabels { kind }).inc();
    }

    pub fn free_packs(&self) {
        self.free_packs.inc();
    }

    fn http(&self, method: &str, route: &str, status: u16, seconds: Option<f64>) {
        let (method, route) = (method.to_owned(), route.to_owned());
        if let Some(s) = seconds {
            self.http_duration.get_or_create(&RouteLabels { method: method.clone(), route: route.clone() }).observe(s);
        }
        self.http_requests.get_or_create(&HttpLabels { method, route, status }).inc();
    }

    /// The text exposition, with the gauges read now
    pub async fn render(&self, state: &AppState) -> String {
        let (sockets, online) = state.live.counts();
        self.sockets.set(sockets as i64);
        self.online.set(online as i64);
        let pool = state.pool.status();
        self.pool_size.set(pool.size as i64);
        self.pool_available.set(pool.available as i64);
        self.pool_waiting.set(pool.waiting as i64);
        if let Ok(mut conn) = state.pool.get().await {
            let users: Result<i64, _> = crate::schema::users::table.count().get_result(&mut conn).await;
            match users {
                Ok(n) => {
                    self.users.set(n);
                }
                Err(e) => tracing::warn!("metrics: counting users: {e}"),
            }
        }
        let mut out = String::new();
        prometheus_client::encoding::text::encode(&mut out, &self.registry).expect("writing to a String");
        out
    }
}

/// Counts and times every request by route pattern. Wrap the App with
/// `actix_web::middleware::from_fn(metrics::track)`.
pub async fn track(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse, actix_web::Error> {
    let start = Instant::now();
    let state = req.app_data::<web::Data<AppState>>().cloned();
    let method = req.method().to_string();
    let res = next.call(req).await?;
    if let Some(state) = state {
        // Unknown paths share one label: scanners cannot grow the series
        let route = res.request().match_pattern().unwrap_or_else(|| "unmatched".to_owned());
        // A live socket stays open for hours: its time says nothing about latency
        let seconds = (route != "/api/live").then(|| start.elapsed().as_secs_f64());
        state.metrics.http(&method, &route, res.status().as_u16(), seconds);
    }
    Ok(res.map_into_boxed_body())
}

/// Compares without stopping at the first difference
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[get("/metrics")]
pub async fn scrape(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    if let Some(token) = &state.config.metrics_token {
        let given =
            req.headers().get("authorization").and_then(|h| h.to_str().ok()).and_then(|h| h.strip_prefix("Bearer "));
        if !given.is_some_and(|g| constant_time_eq(g.as_bytes(), token.as_bytes())) {
            return HttpResponse::Unauthorized().insert_header(("www-authenticate", "Bearer")).finish();
        }
    }
    HttpResponse::Ok()
        .content_type("application/openmetrics-text; version=1.0.0; charset=utf-8")
        .body(state.metrics.render(&state).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(m: &Metrics) -> String {
        let mut out = String::new();
        prometheus_client::encoding::text::encode(&mut out, &m.registry).unwrap();
        out
    }

    #[test]
    fn counters_show_with_their_labels() {
        let m = Metrics::default();
        m.booster_bought("sweets");
        m.booster_bought("sweets");
        m.card("pack", Rarity::Legendary, true);
        m.spent("booster", 200);
        m.auth("login", "limited");
        m.http("GET", "/api/trades/{id}", 200, Some(0.01));
        let t = text(&m);
        assert!(t.contains(r#"anycardz_boosters_bought_total{collection="sweets"} 2"#), "{t}");
        assert!(t.contains(r#"anycardz_cards_obtained_total{source="pack",rarity="legendary",foil="yes"} 1"#), "{t}");
        assert!(t.contains(r#"anycardz_coins_spent_total{reason="booster"} 200"#), "{t}");
        assert!(t.contains(r#"anycardz_auth_attempts_total{action="login",result="limited"} 1"#), "{t}");
        assert!(t.contains(r#"anycardz_http_requests_total{method="GET",route="/api/trades/{id}",status="200"} 1"#));
        assert!(t.contains(r#"anycardz_http_request_duration_seconds_count{method="GET",route="/api/trades/{id}"} 1"#));
    }

    #[test]
    fn negative_amounts_never_lower_a_counter() {
        let m = Metrics::default();
        m.earned("sell", -5);
        assert!(text(&m).contains(r#"anycardz_coins_earned_total{reason="sell"} 0"#));
    }
}
