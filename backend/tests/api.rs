//! End-to-end tests of the HTTP API against a real Postgres.
//! They need TEST_DATABASE_URL (see .env.example) and are skipped without it.

use std::sync::Arc;

use actix_web::cookie::Cookie;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::{App, test, web};
use anycardz_api::{AppState, Catalog, Config, db, routes, schema, test_support};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde_json::{Value, json};

struct Ctx {
    state: web::Data<AppState>,
    _cache: tempfile::TempDir,
}

async fn ctx() -> Option<Ctx> {
    let _ = dotenvy::dotenv();
    // Internal errors are logged: a failing test shows them
    let _ = tracing_subscriber::fmt().with_test_writer().with_env_filter("anycardz_api=error").try_init();
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("TEST_DATABASE_URL not set, skipping");
        return None;
    };
    // Tests run in parallel: migrate once per process
    static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
    MIGRATED.get_or_init(|| async { db::run_migrations(&url).await.unwrap() }).await;
    let cache = tempfile::tempdir().unwrap();
    let config = Config {
        database_url: url.clone(),
        data_dir: test_support::fixture_dir(),
        cache_dir: cache.path().to_owned(),
        bind: String::new(),
        dev_collections: false,
        secure_cookies: false,
        trusted_proxies: Vec::new(),
    };
    let catalog = Catalog::load(&config.data_dir, false).unwrap();
    let state = AppState {
        pool: db::pool(&url).unwrap(),
        live: Default::default(),
        limits: anycardz_api::ratelimit::AuthLimits::relaxed(),
        catalog: Arc::new(catalog),
        config: Arc::new(config),
    };
    Some(Ctx { state: web::Data::new(state), _cache: cache })
}

macro_rules! app {
    ($ctx:expr) => {
        test::init_service(App::new().app_data($ctx.state.clone()).configure(routes::configure)).await
    };
}

fn unique(prefix: &str) -> String {
    format!("{prefix}{}", rand::random::<u32>())
}

fn session(res: &ServiceResponse) -> Cookie<'static> {
    res.response().cookies().find(|c| c.name() == "session").expect("session cookie").into_owned()
}

async fn body(res: ServiceResponse) -> Value {
    serde_json::from_slice(&test::read_body(res).await).unwrap_or(Value::Null)
}

async fn register<S>(app: &S, name: &str) -> Cookie<'static>
where
    S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let req = test::TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({ "username": name, "email": format!("{name}@example.com"), "password": "123456789" }))
        .to_request();
    let res = test::call_service(app, req).await;
    assert_eq!(res.status(), 201);
    session(&res)
}

async fn get<S>(app: &S, uri: &str, cookie: &Cookie<'static>) -> (u16, Value)
where
    S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let res = test::call_service(app, test::TestRequest::get().uri(uri).cookie(cookie.clone()).to_request()).await;
    let status = res.status().as_u16();
    (status, body(res).await)
}

async fn post<S>(app: &S, uri: &str, cookie: &Cookie<'static>, payload: Value) -> (u16, Value)
where
    S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let req = test::TestRequest::post().uri(uri).cookie(cookie.clone()).set_json(payload).to_request();
    let res = test::call_service(app, req).await;
    let status = res.status().as_u16();
    (status, body(res).await)
}

#[actix_web::test]
async fn register_login_logout() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let name = unique("user");
    let cookie = register(&app, &name).await;

    let (status, me) = get(&app, "/api/me", &cookie).await;
    assert_eq!(status, 200);
    assert_eq!(me["username"], name);
    assert_eq!(me["coins"], 1000);
    assert_eq!(me["pile"], json!([]));
    assert_eq!(me["streak"]["day"], 1);
    assert_eq!(me["streak"]["claimed_today"], false);

    // Same name in another case is taken
    let req = test::TestRequest::post()
        .uri("/api/auth/register")
        .set_json(json!({ "username": name.to_uppercase(), "email": format!("other{name}@example.com"), "password": "123456789" }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), 409);
    assert_eq!(body(res).await["error"], "username_taken");

    let anon = Cookie::new("none", "");
    let (status, err) =
        post(&app, "/api/auth/login", &anon, json!({ "login": name, "password": "wrong-password" })).await;
    assert_eq!((status, err["error"].as_str()), (422, Some("invalid_credentials")));

    // Login by email, any case
    let req = test::TestRequest::post()
        .uri("/api/auth/login")
        .set_json(json!({ "login": format!("{}@EXAMPLE.com", name.to_uppercase()), "password": "123456789" }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), 200);
    let second = session(&res);

    let (status, _) = post(&app, "/api/auth/logout", &second, json!({})).await;
    assert_eq!(status, 200);
    assert_eq!(get(&app, "/api/me", &second).await.0, 401, "logged out session is gone");
    assert_eq!(get(&app, "/api/me", &cookie).await.0, 200, "other sessions survive");
}

#[actix_web::test]
async fn validation_errors() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let anon = Cookie::new("none", "");
    for (payload, code) in [
        (json!({ "username": "x", "email": "a@b.co", "password": "123456789" }), "invalid_username"),
        (json!({ "username": unique("u"), "email": "nope", "password": "123456789" }), "invalid_email"),
        (json!({ "username": unique("u"), "email": "a@b.co", "password": "short" }), "weak_password"),
        (json!({ "username": 3 }), "invalid_body"),
    ] {
        let (status, err) = post(&app, "/api/auth/register", &anon, payload).await;
        assert_eq!((status, err["error"].as_str()), (422, Some(code)));
    }
    assert_eq!(get(&app, "/api/me", &anon).await.0, 401);
}

#[actix_web::test]
async fn concurrent_purchases_never_overspend() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let cookie = register(&app, &unique("buyer")).await;

    // 1000 coins buy exactly four 250-coin boosters, however the requests race
    let buys = (0..8).map(|_| post(&app, "/api/shop/boosters", &cookie, json!({ "collection": "minerals" })));
    let results = futures_util::future::join_all(buys).await;
    assert_eq!(results.iter().filter(|(s, _)| *s == 200).count(), 4);
    assert!(results.iter().filter(|(s, _)| *s != 200).all(|(s, b)| *s == 409 && b["error"] == "not_enough_coins"));

    let (_, me) = get(&app, "/api/me", &cookie).await;
    assert_eq!(me["coins"], 0);
    assert_eq!(me["pile"].as_array().unwrap().len(), 4);

    let (status, err) = post(&app, "/api/shop/boosters", &cookie, json!({ "collection": "nope" })).await;
    assert_eq!((status, err["error"].as_str()), (404, Some("unknown_collection")));
}

#[actix_web::test]
async fn open_pack_then_sell() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let cookie = register(&app, &unique("opener")).await;

    let (status, err) = post(&app, "/api/pile/open", &cookie, json!({})).await;
    assert_eq!((status, err["error"].as_str()), (409, Some("no_booster")));

    post(&app, "/api/shop/boosters", &cookie, json!({ "collection": "minerals" })).await;
    let (status, pack) = post(&app, "/api/pile/open", &cookie, json!({})).await;
    assert_eq!(status, 200);
    assert_eq!(pack["collection"], "minerals");
    assert_eq!(pack["pile"], json!([]));
    let cards = pack["cards"].as_array().unwrap();
    assert_eq!(cards.len(), 5);
    let first = &cards[0];
    assert_eq!(first["is_new"], true, "the very first card is always new");

    let (_, binder) = get(&app, "/api/binder", &cookie).await;
    let owned = binder["cards"].as_array().unwrap();
    assert_eq!(owned.iter().map(|c| c["count"].as_i64().unwrap()).sum::<i64>(), 5);
    assert_eq!(binder["collections"]["minerals"].as_u64().unwrap() as usize, owned.len());

    // Sell a plain copy of the first card (fall back to foil when it is one)
    let foil = first["foil"].is_string();
    let (status, sold) = post(&app, "/api/binder/sell", &cookie, json!({ "card": first["card"], "foil": foil })).await;
    assert_eq!(status, 200);
    let price = sold["sold_for"].as_i64().unwrap();
    assert!(price >= 10);
    assert_eq!(sold["coins"].as_i64().unwrap(), 750 + price);

    let (status, err) = post(&app, "/api/binder/sell", &cookie, json!({ "card": "diamond", "foil": true })).await;
    assert!(status == 409 && err["error"] == "not_owned" || status == 200, "only owned copies sell");
}

#[actix_web::test]
async fn streak_claims_once_a_day() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let cookie = register(&app, &unique("streaker")).await;

    let claims = (0..3).map(|_| post(&app, "/api/streak/claim", &cookie, json!({})));
    let results = futures_util::future::join_all(claims).await;
    assert_eq!(results.iter().filter(|(s, _)| *s == 200).count(), 1, "a double tap claims once");
    let ok = results.iter().find(|(s, _)| *s == 200).unwrap();
    assert_eq!(ok.1["reward"], 200);
    assert_eq!(ok.1["coins"], 1200);
    assert_eq!(ok.1["streak"]["claimed_today"], true);
    assert_eq!(ok.1["streak"]["next_reward"], 300);
}

#[actix_web::test]
async fn catalog_is_localized_and_serves_thumbnails() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let fr = Cookie::new("lang", "fr");

    let (status, list) = get(&app, "/api/collections", &fr).await;
    assert_eq!(status, 200);
    assert_eq!(list[0]["title"], "Minéraux");
    assert_eq!(list[0]["featured"]["slug"], "diamond");
    assert_eq!(list[0]["price"], 250);

    let (_, col) = get(&app, "/api/collections/minerals", &fr).await;
    assert_eq!(col["cards"].as_array().unwrap().len(), 6);
    assert_eq!(col["cards"][0]["title"], "Quartz (fr)");
    assert_eq!(col["cards"][0]["description"], "A common mineral.", "falls back to English");

    let res = test::call_service(&app, test::TestRequest::get().uri("/api/cards/quartz/art?w=420").to_request()).await;
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers().get("content-type").unwrap(), "image/jpeg");
    let res = test::call_service(&app, test::TestRequest::get().uri("/api/cards/quartz/art?w=13").to_request()).await;
    assert_eq!(res.status(), 422);
    let res = test::call_service(&app, test::TestRequest::get().uri("/api/assets/..%2Fcards").to_request()).await;
    assert_eq!(res.status(), 404);
    let res = test::call_service(&app, test::TestRequest::get().uri("/api/assets/foil1.png").to_request()).await;
    assert_eq!(res.status(), 200);
}

#[actix_web::test]
async fn topup_grants_coins_without_checkout() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let cookie = register(&app, &unique("topper")).await;

    let (status, list) = get(&app, "/api/shop/topups", &cookie).await;
    assert_eq!(status, 200);
    assert_eq!(list["checkout"], false);
    assert_eq!(list["packs"][1]["id"], "medium");
    assert_eq!(list["packs"][1]["coins"], 500);

    let (status, res) = post(&app, "/api/shop/topup", &cookie, json!({ "pack": "medium" })).await;
    assert_eq!(status, 200);
    assert_eq!((res["coins"].as_i64(), res["added"].as_i64()), (Some(1500), Some(500)));

    let (status, err) = post(&app, "/api/shop/topup", &cookie, json!({ "pack": "free-money" })).await;
    assert_eq!((status, err["error"].as_str()), (404, Some("unknown_pack")));
    let anon = Cookie::new("none", "");
    assert_eq!(post(&app, "/api/shop/topup", &anon, json!({ "pack": "small" })).await.0, 401);
}

/// Puts copies straight into a player's binder
async fn give(ctx: &Ctx, user_id: i64, card: &str, foil: Option<&str>, n: usize) {
    use schema::owned_cards::dsl as oc;
    let mut conn = ctx.state.pool.get().await.unwrap();
    let rows: Vec<_> = (0..n).map(|_| (oc::user_id.eq(user_id), oc::card.eq(card), oc::foil.eq(foil))).collect();
    diesel::insert_into(oc::owned_cards).values(rows).execute(&mut conn).await.unwrap();
}

async fn owned<S>(app: &S, cookie: &Cookie<'static>) -> std::collections::HashMap<String, (i64, i64)>
where
    S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (_, b) = get(app, "/api/binder", cookie).await;
    b["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["card"].as_str().unwrap().to_owned(), (c["count"].as_i64().unwrap(), c["foils"].as_i64().unwrap())))
        .collect()
}

#[actix_web::test]
async fn tradeup_rules() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let cookie = register(&app, &unique("forger")).await;
    let uid = get(&app, "/api/me", &cookie).await.1["id"].as_i64().unwrap();
    give(&ctx, uid, "quartz", None, 6).await;
    give(&ctx, uid, "mica", None, 4).await;
    give(&ctx, uid, "garnet", None, 1).await;
    give(&ctx, uid, "diamond", None, 10).await;

    for (cards, code, status) in [
        (json!([{ "card": "quartz", "count": 6 }]), "tradeup_count", 422),
        (
            json!([{ "card": "quartz", "count": 6 }, { "card": "mica", "count": 3 }, { "card": "garnet" }]),
            "tradeup_rarity",
            422,
        ),
        (json!([{ "card": "diamond", "count": 10 }]), "tradeup_tier", 422),
        (json!([{ "card": "quartz", "count": 6 }, { "card": "feldspar", "count": 4 }]), "not_owned", 409),
        (json!([{ "card": "nope", "count": 10 }]), "unknown_card", 404),
        (json!([{ "card": "quartz", "count": 0 }]), "invalid_count", 422),
    ] {
        let (s, err) = post(&app, "/api/forge/tradeup", &cookie, json!({ "cards": cards })).await;
        assert_eq!((s, err["error"].as_str()), (status, Some(code)), "{cards}");
    }
    assert_eq!(owned(&app, &cookie).await["quartz"], (6, 0), "refused trade-ups take nothing");
    let (_, b) = get(&app, "/api/binder?details=1", &cookie).await;
    assert_eq!(b["cards"].as_array().unwrap().iter().find(|c| c["card"] == "quartz").unwrap()["patterns"], json!([]));
    assert_eq!(b["details"]["garnet"]["rarity"], "rare");
    assert_eq!(get(&app, "/api/binder", &cookie).await.1["details"], Value::Null);

    let (s, res) = post(
        &app,
        "/api/forge/tradeup",
        &cookie,
        json!({ "cards": [{ "card": "quartz", "count": 6 }, { "card": "mica", "count": 4 }] }),
    )
    .await;
    assert_eq!(s, 200, "{res}");
    assert_eq!(res["rarity"], "rare");
    assert!(["garnet", "opal"].contains(&res["card"].as_str().unwrap()));
    assert_eq!(res["odds"], json!([{ "collection": "minerals", "chance": 10000 }]));
    let after = owned(&app, &cookie).await;
    assert!(!after.contains_key("quartz") && !after.contains_key("mica"), "the 10 commons are spent");
    let won = res["card"].as_str().unwrap();
    assert_eq!(after[won].0, res["copies"].as_i64().unwrap());
}

#[actix_web::test]
async fn concurrent_tradeups_spend_cards_once() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let cookie = register(&app, &unique("racer")).await;
    let uid = get(&app, "/api/me", &cookie).await.1["id"].as_i64().unwrap();
    give(&ctx, uid, "quartz", None, 10).await;
    let body = json!({ "cards": [{ "card": "quartz", "count": 10 }] });
    let results =
        futures_util::future::join_all((0..3).map(|_| post(&app, "/api/forge/tradeup", &cookie, body.clone()))).await;
    assert_eq!(results.iter().filter(|(s, _)| *s == 200).count(), 1, "{results:?}");
    assert!(results.iter().filter(|(s, _)| *s != 200).all(|(s, b)| *s == 409 && b["error"] == "not_owned"));
}

#[actix_web::test]
async fn roulette_burns_and_wins_or_destroys() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let cookie = register(&app, &unique("gambler")).await;
    let uid = get(&app, "/api/me", &cookie).await.1["id"].as_i64().unwrap();

    let (s, err) = post(&app, "/api/forge/roulette", &cookie, json!({ "target": "diamond" })).await;
    assert_eq!((s, err["error"].as_str()), (409, Some("not_owned")), "needs a plain copy of the target");
    give(&ctx, uid, "quartz", None, 101).await;
    let (s, err) = post(
        &app,
        "/api/forge/roulette",
        &cookie,
        json!({ "target": "quartz", "burn": [{ "card": "quartz", "count": 101 }] }),
    )
    .await;
    assert_eq!((s, err["error"].as_str()), (422, Some("roulette_count")));

    // Many spins: every one must burn its cards, and win (foil) or lose (destroyed)
    give(&ctx, uid, "diamond", None, 40).await;
    give(&ctx, uid, "garnet", None, 80).await;
    let (mut wins, mut losses) = (0, 0);
    for _ in 0..40 {
        let before = owned(&app, &cookie).await;
        let (s, res) = post(
            &app,
            "/api/forge/roulette",
            &cookie,
            json!({ "target": "diamond", "burn": [{ "card": "garnet", "count": 2 }] }),
        )
        .await;
        assert_eq!(s, 200, "{res}");
        assert_eq!(res["chance"], 500 + 2 * 100);
        let after = owned(&app, &cookie).await;
        assert_eq!(after.get("garnet").map(|g| g.0).unwrap_or(0), before["garnet"].0 - 2, "burned either way");
        let (d0, f0) = before["diamond"];
        let (d1, f1) = after.get("diamond").copied().unwrap_or((0, 0));
        if res["won"] == true {
            wins += 1;
            assert!(res["foil"].as_str().unwrap().starts_with("foil"));
            assert_eq!((d1, f1), (d0, f0 + 1), "won: the copy turns foil");
        } else {
            losses += 1;
            assert_eq!((d1, f1), (d0 - 1, f0), "lost: the copy is destroyed");
        }
    }
    assert!(losses > 0, "7% odds should lose some of 40 spins");
    let _ = wins;
}

async fn put<S>(app: &S, uri: &str, cookie: &Cookie<'static>, payload: Value) -> (u16, Value)
where
    S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let req = test::TestRequest::put().uri(uri).cookie(cookie.clone()).set_json(payload).to_request();
    let res = test::call_service(app, req).await;
    let status = res.status().as_u16();
    (status, body(res).await)
}

/// Two players, each with an id
async fn pair<S>(app: &S) -> ((String, Cookie<'static>, i64), (String, Cookie<'static>, i64))
where
    S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let mut out = Vec::new();
    for p in ["alice", "bob"] {
        let name = unique(p);
        let cookie = register(app, &name).await;
        let id = get(app, "/api/me", &cookie).await.1["id"].as_i64().unwrap();
        out.push((name, cookie, id));
    }
    let b = out.pop().unwrap();
    (out.pop().unwrap(), b)
}

#[actix_web::test]
async fn trade_swaps_cards_and_coins_once_both_seal() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let ((_, a, a_id), (b_name, b, b_id)) = pair(&app).await;
    give(&ctx, a_id, "garnet", Some("foil4"), 1).await;
    give(&ctx, a_id, "quartz", None, 2).await;
    give(&ctx, b_id, "diamond", None, 1).await;

    let (s, t) = post(&app, "/api/trades", &a, json!({ "with": b_name.to_uppercase() })).await;
    assert_eq!(s, 201, "{t}");
    let id = t["id"].as_i64().unwrap();
    let (s, again) = post(&app, "/api/trades", &a, json!({ "with": b_name })).await;
    assert_eq!((s, again["id"].as_i64()), (200, Some(id)), "one open trade per pair");

    // Empty trades cannot be sealed
    let (s, err) = post(&app, &format!("/api/trades/{id}/seal"), &a, json!({ "version": 1 })).await;
    assert_eq!((s, err["error"].as_str()), (422, Some("trade_empty")));

    let offer = json!({ "version": 1, "give": [{ "card": "garnet", "foil": true }, { "card": "quartz", "count": 2 }],
        "get": [{ "card": "diamond" }], "give_coins": 0, "get_coins": 100 });
    let (s, t) = put(&app, &format!("/api/trades/{id}"), &a, offer).await;
    assert_eq!(s, 200, "{t}");
    assert_eq!(t["version"], 2);
    assert_eq!(t["edited_by"], "me");
    assert_eq!(t["details"]["diamond"]["rarity"], "legendary");

    // Bob sees it from his side
    let (_, bt) = get(&app, &format!("/api/trades/{id}"), &b).await;
    assert_eq!(bt["mine"]["cards"], json!([{ "card": "diamond", "foil": false, "count": 1 }]));
    assert_eq!(bt["mine"]["coins"], 100);
    assert_eq!(bt["edited_by"], "partner");

    let (s, t) = post(&app, &format!("/api/trades/{id}/seal"), &a, json!({ "version": 2 })).await;
    assert_eq!((s, t["mine"]["sealed"].as_bool(), t["state"].as_str()), (200, Some(true), Some("open")));
    let (s, t) = post(&app, &format!("/api/trades/{id}/seal"), &b, json!({ "version": 2 })).await;
    assert_eq!((s, t["state"].as_str()), (200, Some("done")), "{t}");

    let a_has = owned(&app, &a).await;
    let b_has = owned(&app, &b).await;
    // The foil keeps its pattern, and the binder says which
    let (_, bb) = get(&app, "/api/binder", &b).await;
    let garnet = bb["cards"].as_array().unwrap().iter().find(|c| c["card"] == "garnet").unwrap().clone();
    assert_eq!(garnet["patterns"], json!(["foil4"]));
    let (_, pc) = get(&app, &format!("/api/players/{b_name}/cards"), &a).await;
    assert_eq!(
        pc["cards"].as_array().unwrap().iter().find(|c| c["card"] == "garnet").unwrap()["patterns"],
        json!(["foil4"])
    );
    assert_eq!(a_has.get("diamond"), Some(&(1, 0)));
    assert!(!a_has.contains_key("garnet") && !a_has.contains_key("quartz"));
    assert_eq!(b_has.get("garnet"), Some(&(1, 1)), "the foil stays foil");
    assert_eq!(b_has.get("quartz"), Some(&(2, 0)));
    assert!(!b_has.contains_key("diamond"));
    assert_eq!(get(&app, "/api/me", &a).await.1["coins"], 1100);
    assert_eq!(get(&app, "/api/me", &b).await.1["coins"], 900);

    let (s, err) = post(&app, &format!("/api/trades/{id}/seal"), &a, json!({ "version": 2 })).await;
    assert_eq!((s, err["error"].as_str()), (409, Some("trade_closed")));
}

#[actix_web::test]
async fn trade_edits_break_seals() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let ((_, a, a_id), (b_name, b, b_id)) = pair(&app).await;
    give(&ctx, a_id, "quartz", None, 1).await;
    give(&ctx, b_id, "diamond", None, 1).await;
    give(&ctx, b_id, "mica", None, 1).await;
    let id = post(&app, "/api/trades", &a, json!({ "with": b_name })).await.1["id"].as_i64().unwrap();
    let uri = format!("/api/trades/{id}");
    put(&app, &uri, &a, json!({ "version": 1, "give": [{ "card": "quartz" }], "get": [{ "card": "diamond" }] })).await;
    post(&app, &format!("{uri}/seal"), &a, json!({ "version": 2 })).await;

    // Bob swaps the diamond for a mica after Alice sealed
    let (s, t) =
        put(&app, &uri, &b, json!({ "version": 2, "give": [{ "card": "mica" }], "get": [{ "card": "quartz" }] })).await;
    assert_eq!(s, 200);
    assert_eq!(
        (t["mine"]["sealed"].as_bool(), t["theirs"]["sealed"].as_bool()),
        (Some(false), Some(false)),
        "both seals break"
    );
    // A seal of what Alice agreed to no longer counts
    let (s, err) = post(&app, &format!("{uri}/seal"), &b, json!({ "version": 2 })).await;
    assert_eq!((s, err["error"].as_str()), (409, Some("trade_changed")));
    let (s, err) = put(&app, &uri, &a, json!({ "version": 2, "give": [], "get": [] })).await;
    assert_eq!((s, err["error"].as_str()), (409, Some("trade_changed")), "edits of an old version are refused");
    // Bob seals his new version: no swap until Alice agrees to it
    let (_, t) = post(&app, &format!("{uri}/seal"), &b, json!({ "version": 3 })).await;
    assert_eq!(t["state"], "open");
    assert_eq!(owned(&app, &b).await.get("diamond"), Some(&(1, 0)));

    // Asking for cards the partner does not have is refused early
    let (s, err) = put(&app, &uri, &a, json!({ "version": 3, "get": [{ "card": "opal" }] })).await;
    assert_eq!((s, err["error"].as_str()), (409, Some("trade_not_owned")));
}

#[actix_web::test]
async fn trade_fails_cleanly_when_cards_are_gone() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let ((_, a, a_id), (b_name, b, b_id)) = pair(&app).await;
    give(&ctx, a_id, "quartz", None, 1).await;
    give(&ctx, b_id, "mica", None, 1).await;
    let id = post(&app, "/api/trades", &a, json!({ "with": b_name })).await.1["id"].as_i64().unwrap();
    let uri = format!("/api/trades/{id}");
    put(
        &app,
        &uri,
        &a,
        json!({ "version": 1, "give": [{ "card": "quartz" }], "get": [{ "card": "mica" }], "give_coins": 300 }),
    )
    .await;
    post(&app, &format!("{uri}/seal"), &a, json!({ "version": 2 })).await;
    // Alice sells the quartz before Bob seals
    post(&app, "/api/binder/sell", &a, json!({ "card": "quartz" })).await;
    let (s, err) = post(&app, &format!("{uri}/seal"), &b, json!({ "version": 2 })).await;
    assert_eq!((s, err["error"].as_str()), (409, Some("trade_unavailable")));
    assert_eq!(owned(&app, &b).await.get("mica"), Some(&(1, 0)), "nothing moved");
    assert_eq!(get(&app, "/api/me", &b).await.1["coins"], 1000);

    // Coins are checked too
    give(&ctx, a_id, "quartz", None, 1).await;
    put(&app, &uri, &a, json!({ "version": 2, "give": [{ "card": "quartz" }], "give_coins": 5000 })).await;
    post(&app, &format!("{uri}/seal"), &a, json!({ "version": 3 })).await;
    let (s, err) = post(&app, &format!("{uri}/seal"), &b, json!({ "version": 3 })).await;
    assert_eq!((s, err["error"].as_str()), (409, Some("trade_unavailable")));
}

/// Also the regression test for a deadlock between a swap and the notification
/// of the first seal: several rounds, as it only showed now and then
#[actix_web::test]
async fn simultaneous_seals_swap_once() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    for _ in 0..5 {
        let ((_, a, a_id), (b_name, b, _)) = pair(&app).await;
        give(&ctx, a_id, "quartz", None, 1).await;
        let id = post(&app, "/api/trades", &a, json!({ "with": b_name })).await.1["id"].as_i64().unwrap();
        let uri = format!("/api/trades/{id}");
        put(&app, &uri, &a, json!({ "version": 1, "give": [{ "card": "quartz" }], "give_coins": 10 })).await;
        let seal = format!("{uri}/seal");
        let both = futures_util::future::join_all([
            post(&app, &seal, &a, json!({ "version": 2 })),
            post(&app, &seal, &b, json!({ "version": 2 })),
        ])
        .await;
        assert_eq!((both[0].0, both[1].0), (200, 200), "{both:?}");
        assert_eq!(get(&app, &uri, &a).await.1["state"], "done");
        assert_eq!(owned(&app, &b).await.get("quartz"), Some(&(1, 0)), "exactly one copy moved");
    }
}

#[actix_web::test]
async fn trade_permissions_and_search() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let ((a_name, a, _), (b_name, b, _)) = pair(&app).await;
    let outsider = register(&app, &unique("eve")).await;

    let (s, err) = post(&app, "/api/trades", &a, json!({ "with": a_name })).await;
    assert_eq!((s, err["error"].as_str()), (422, Some("trade_self")));
    let (s, _) = post(&app, "/api/trades", &a, json!({ "with": "nobody-here" })).await;
    assert_eq!(s, 404);
    let id = post(&app, "/api/trades", &a, json!({ "with": b_name })).await.1["id"].as_i64().unwrap();
    assert_eq!(get(&app, &format!("/api/trades/{id}"), &outsider).await.0, 404, "outsiders do not see it");
    let (s, _) = post(&app, &format!("/api/trades/{id}/cancel"), &outsider, json!({})).await;
    assert_eq!(s, 404);

    let (_, list) = get(&app, "/api/trades", &b).await;
    assert_eq!(list[0]["id"].as_i64(), Some(id));
    assert_eq!(list[0]["partner"], a_name);
    let (s, t) = post(&app, &format!("/api/trades/{id}/cancel"), &b, json!({})).await;
    assert_eq!((s, t["state"].as_str()), (200, Some("cancelled")));
    // A new trade can start after a cancelled one
    assert_eq!(post(&app, "/api/trades", &a, json!({ "with": b_name })).await.0, 201);

    let (_, found) = get(&app, &format!("/api/players?q={}", &b_name[..6]), &a).await;
    assert!(found.as_array().unwrap().iter().any(|p| p["username"] == b_name));
    let (_, found) = get(&app, "/api/players?q=%25", &a).await;
    assert!(found.as_array().unwrap().is_empty(), "% is matched literally");
    let (s, cards) = get(&app, &format!("/api/players/{b_name}/cards"), &a).await;
    assert_eq!((s, cards["cards"].as_array().map(|c| c.len())), (200, Some(0)));
}

#[actix_web::test]
async fn shoutbox_rules_and_brags() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let cookie = register(&app, &unique("talker")).await;
    let uid = get(&app, "/api/me", &cookie).await.1["id"].as_i64().unwrap();

    let (s, err) = post(&app, "/api/shoutbox", &cookie, json!({ "body": "   " })).await;
    assert_eq!((s, err["error"].as_str()), (422, Some("empty_message")));
    let (s, err) = post(&app, "/api/shoutbox", &cookie, json!({ "body": "x".repeat(301) })).await;
    assert_eq!((s, err["error"].as_str()), (422, Some("message_too_long")));
    let (s, err) = post(&app, "/api/shoutbox", &cookie, json!({ "body": "look", "card": { "card": "diamond" } })).await;
    assert_eq!((s, err["error"].as_str()), (409, Some("not_owned")), "brags need the card");

    let (s, msg) = post(&app, "/api/shoutbox", &cookie, json!({ "body": "  hello all  " })).await;
    assert_eq!((s, msg["body"].as_str()), (201, Some("hello all")));
    let (s, err) = post(&app, "/api/shoutbox", &cookie, json!({ "body": "again" })).await;
    assert_eq!((s, err["error"].as_str()), (429, Some("slow_down")), "one message every 2 s");

    // A foil brag carries the pattern of the player's own copy
    give(&ctx, uid, "diamond", Some("foil5"), 1).await;
    tokio::time::sleep(std::time::Duration::from_millis(2100)).await;
    let (s, msg) = post(&app, "/api/shoutbox", &cookie, json!({ "card": { "card": "diamond", "foil": true } })).await;
    assert_eq!((s, msg["card"].as_str(), msg["foil"].as_str()), (201, Some("diamond"), Some("foil5")));

    let (_, history) = get(&app, "/api/shoutbox", &cookie).await;
    let mine: Vec<_> = history.as_array().unwrap().iter().filter(|m| m["user"] == msg["user"]).collect();
    assert_eq!(mine.len(), 2);
    assert_eq!(mine[1]["card"], "diamond", "oldest first");

    let (_, cards) = get(&app, "/api/cards?slugs=quartz,diamond,nope", &Cookie::new("lang", "fr")).await;
    assert_eq!(cards["quartz"]["title"], "Quartz (fr)");
    assert!(cards.get("nope").is_none());
}

#[actix_web::test]
async fn live_socket_pushes_events() {
    use futures_util::{SinkExt as _, StreamExt as _};
    let Some(ctx) = ctx().await else { return };
    let state = ctx.state.clone();
    let srv = actix_test::start(move || App::new().app_data(state.clone()).configure(routes::configure));

    // Two players over real HTTP, to get cookies the socket can use
    let client = awc::Client::default();
    let mut players = Vec::new();
    for p in ["sock_a", "sock_b"] {
        let name = unique(p);
        let res = client
            .post(srv.url("/api/auth/register"))
            .send_json(&json!({ "username": name, "email": format!("{name}@example.com"), "password": "123456789" }))
            .await
            .unwrap();
        let cookie = res.cookie("session").unwrap().into_owned();
        players.push((name, cookie));
    }
    let connect = |cookie: Cookie<'static>| {
        let url = srv.url("/api/live");
        async move {
            let (_, ws) = awc::Client::new().ws(url).cookie(cookie).connect().await.unwrap();
            ws
        }
    };
    let mut ws_a = connect(players[0].1.clone()).await;
    let mut ws_b = connect(players[1].1.clone()).await;
    async fn next_json(ws: &mut actix_codec::Framed<awc::BoxedSocket, awc::ws::Codec>) -> Value {
        loop {
            let frame = tokio::time::timeout(std::time::Duration::from_secs(5), ws.next())
                .await
                .expect("event")
                .unwrap()
                .unwrap();
            if let awc::ws::Frame::Text(t) = frame {
                return serde_json::from_slice(&t).unwrap();
            }
        }
    }
    let hello = next_json(&mut ws_a).await;
    assert_eq!(hello["type"], "hello");
    assert!(hello["messages"].is_array() && hello["pulls"].is_array());
    next_json(&mut ws_b).await;

    // A chat message reaches everyone
    let res = client
        .post(srv.url("/api/shoutbox"))
        .cookie(players[0].1.clone())
        .send_json(&json!({ "body": "over the wire" }))
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    for ws in [&mut ws_a, &mut ws_b] {
        let ev = next_json(ws).await;
        assert_eq!((ev["type"].as_str(), ev["body"].as_str()), (Some("chat"), Some("over the wire")));
    }

    // Trade pings reach the two players only: a third socket sees the next chat first
    let outsider = register(
        &test::init_service(App::new().app_data(ctx.state.clone()).configure(routes::configure)).await,
        &unique("sock_c"),
    )
    .await;
    let mut ws_c = connect(outsider).await;
    next_json(&mut ws_c).await;
    let res = client
        .post(srv.url("/api/trades"))
        .cookie(players[0].1.clone())
        .send_json(&json!({ "with": players[1].0 }))
        .await
        .unwrap();
    assert_eq!(res.status(), 201);
    for ws in [&mut ws_a, &mut ws_b] {
        assert_eq!(next_json(ws).await["type"], "trade");
    }
    let note = next_json(&mut ws_b).await;
    assert_eq!((note["type"].as_str(), note["kind"].as_str()), (Some("notification"), Some("trade_opened")));
    assert_eq!(note["actor"], players[0].0.as_str());
    ctx.state.live.pull(anycardz_api::live::PullEvent {
        user: "x".into(),
        card: "diamond".into(),
        rarity: anycardz_api::data::Rarity::Legendary,
        foil: None,
        source: anycardz_api::live::PullSource::Pack,
        at: chrono::Utc::now(),
    });
    assert_eq!(next_json(&mut ws_c).await["type"], "pull", "no trade event before it");
    let _ = ws_a.close().await;
}

#[actix_web::test]
async fn profiles_and_avatars() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let ((a_name, a, a_id), (_, b, _)) = pair(&app).await;
    give(&ctx, a_id, "quartz", None, 2).await;
    give(&ctx, a_id, "diamond", Some("foil1"), 1).await;

    let (s, err) = put(&app, "/api/me/profile", &a, json!({ "bio": "x".repeat(281) })).await;
    assert_eq!((s, err["error"].as_str()), (422, Some("bio_too_long")));
    let (s, _) = put(&app, "/api/me/profile", &a, json!({ "bio": "  I collect minerals  " })).await;
    assert_eq!(s, 200);

    let (s, p) = get(&app, &format!("/api/players/{}", a_name.to_uppercase()), &b).await;
    assert_eq!(s, 200);
    assert_eq!(p["bio"], "I collect minerals");
    assert_eq!((p["cards"].as_u64(), p["copies"].as_i64(), p["foils"].as_i64()), (Some(2), Some(3), Some(1)));
    assert_eq!(p["collections"][0], json!({ "slug": "minerals", "owned": 2, "total": 6 }));
    assert_eq!(p["relation"], "none");
    assert_eq!(p["avatar"], Value::Null);
    assert_eq!(get(&app, &format!("/api/players/{a_name}"), &a).await.1["relation"], "self");

    // Avatar: any image, cropped to a 256 px square JPEG
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbImage::from_pixel(400, 200, image::Rgb([200, 30, 30]))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let req =
        test::TestRequest::put().uri("/api/me/avatar").cookie(a.clone()).set_payload(png.into_inner()).to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), 200);
    let url = body(res).await["avatar"].as_str().unwrap().to_owned();
    assert!(url.starts_with(&format!("/api/avatars/{a_name}?v=")));
    let res = test::call_service(&app, test::TestRequest::get().uri(&url).to_request()).await;
    assert_eq!(res.headers().get("content-type").unwrap(), "image/jpeg");
    let img = image::load_from_memory(&test::read_body(res).await).unwrap();
    assert_eq!((img.width(), img.height()), (256, 256));
    assert_eq!(get(&app, "/api/me", &a).await.1["avatar"], url.as_str());

    let req = test::TestRequest::put().uri("/api/me/avatar").cookie(a.clone()).set_payload("not an image").to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!((res.status().as_u16(), body(res).await["error"].clone()), (422, json!("invalid_image")));
    let req = test::TestRequest::delete().uri("/api/me/avatar").cookie(a.clone()).to_request();
    assert_eq!(test::call_service(&app, req).await.status(), 200);
    assert_eq!(test::call_service(&app, test::TestRequest::get().uri(&url).to_request()).await.status(), 404);
}

async fn del<S>(app: &S, uri: &str, cookie: &Cookie<'static>) -> (u16, Value)
where
    S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let res = test::call_service(app, test::TestRequest::delete().uri(uri).cookie(cookie.clone()).to_request()).await;
    let status = res.status().as_u16();
    (status, body(res).await)
}

#[actix_web::test]
async fn friend_requests() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let ((a_name, a, _), (b_name, b, _)) = pair(&app).await;

    let (s, r) = post(&app, &format!("/api/friends/{b_name}"), &a, json!({})).await;
    assert_eq!((s, r["relation"].as_str()), (200, Some("request_sent")));
    assert_eq!(get(&app, &format!("/api/players/{a_name}"), &b).await.1["relation"], "request_received");
    assert_eq!(get(&app, "/api/me", &b).await.1["friend_requests"], 1);
    let (_, fb) = get(&app, "/api/friends", &b).await;
    assert_eq!(fb["incoming"][0]["username"], a_name);
    let (_, fa) = get(&app, "/api/friends", &a).await;
    assert_eq!(fa["outgoing"][0]["username"], b_name);

    // Asking back accepts
    let (_, r) = post(&app, &format!("/api/friends/{a_name}"), &b, json!({})).await;
    assert_eq!(r["relation"], "friends");
    let (_, fa) = get(&app, "/api/friends", &a).await;
    assert_eq!(
        (fa["friends"][0]["username"].as_str(), fa["friends"][0]["online"].as_bool()),
        (Some(b_name.as_str()), Some(false))
    );
    assert_eq!(fa["outgoing"], json!([]));

    let (_, r) = del(&app, &format!("/api/friends/{a_name}"), &b).await;
    assert_eq!(r["relation"], "none");
    let (s, err) = post(&app, &format!("/api/friends/{a_name}"), &a, json!({})).await;
    assert_eq!((s, err["error"].as_str()), (422, Some("friend_self")));
}

#[actix_web::test]
async fn blocking_cuts_contact() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let ((a_name, a, _), (b_name, b, _)) = pair(&app).await;
    post(&app, &format!("/api/friends/{b_name}"), &a, json!({})).await;
    post(&app, &format!("/api/friends/{a_name}"), &b, json!({})).await;
    let words = format!("hi from {b_name}");
    post(&app, "/api/shoutbox", &b, json!({ "body": words })).await;

    let (_, r) = post(&app, &format!("/api/blocks/{b_name}"), &a, json!({})).await;
    assert_eq!(r["relation"], "blocked");
    assert_eq!(get(&app, "/api/friends", &a).await.1["friends"], json!([]), "the friendship ends");
    assert_eq!(get(&app, "/api/blocks", &a).await.1, json!([{ "username": b_name }]));

    // Neither can reach the other
    for (who, other) in [(&a, &b_name), (&b, &a_name)] {
        let (s, err) = post(&app, &format!("/api/friends/{other}"), who, json!({})).await;
        assert_eq!((s, err["error"].as_str()), (409, Some("player_unavailable")));
        let (s, err) = post(&app, "/api/trades", who, json!({ "with": other })).await;
        assert_eq!((s, err["error"].as_str()), (409, Some("player_unavailable")));
    }
    // Alice no longer sees Bob's messages; others still do
    let (_, history) = get(&app, "/api/shoutbox", &a).await;
    assert!(!history.as_array().unwrap().iter().any(|m| m["body"] == words.as_str()));
    let (_, history) = get(&app, "/api/shoutbox", &b).await;
    assert!(history.as_array().unwrap().iter().any(|m| m["body"] == words.as_str()));

    let (_, r) = del(&app, &format!("/api/blocks/{b_name}"), &a).await;
    assert_eq!(r["relation"], "none");
    assert_eq!(post(&app, "/api/trades", &a, json!({ "with": b_name })).await.0, 201);
}

async fn notes<S>(app: &S, cookie: &Cookie<'static>) -> (Vec<String>, i64)
where
    S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let (_, n) = get(app, "/api/notifications", cookie).await;
    let kinds = n["items"].as_array().unwrap().iter().map(|i| i["kind"].as_str().unwrap().to_owned()).collect();
    (kinds, n["unread"].as_i64().unwrap())
}

#[actix_web::test]
async fn notifications_for_friends_and_trades() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let ((a_name, a, a_id), (b_name, b, _)) = pair(&app).await;

    post(&app, &format!("/api/friends/{b_name}"), &a, json!({})).await;
    post(&app, &format!("/api/friends/{b_name}"), &a, json!({})).await;
    assert_eq!(notes(&app, &b).await, (vec!["friend_request".to_owned()], 1), "asking twice notifies once");
    post(&app, &format!("/api/friends/{a_name}"), &b, json!({})).await;
    assert_eq!(notes(&app, &a).await.0, ["friend_accepted"]);
    let (_, n) = get(&app, "/api/notifications", &a).await;
    assert_eq!(
        (n["items"][0]["actor"].as_str(), n["items"][0]["read"].as_bool()),
        (Some(b_name.as_str()), Some(false))
    );

    give(&ctx, a_id, "quartz", None, 2).await;
    let id = post(&app, "/api/trades", &a, json!({ "with": b_name })).await.1["id"].as_i64().unwrap();
    let uri = format!("/api/trades/{id}");
    put(&app, &uri, &a, json!({ "version": 1, "give": [{ "card": "quartz" }] })).await;
    put(&app, &uri, &a, json!({ "version": 2, "give": [{ "card": "quartz", "count": 2 }] })).await;
    let (kinds, unread) = notes(&app, &b).await;
    assert_eq!(kinds, ["trade_changed", "trade_opened", "friend_request"], "edits fold into one notification");
    assert_eq!(unread, 3);
    let (_, n) = get(&app, "/api/notifications", &b).await;
    assert_eq!(n["items"][0]["trade"].as_i64(), Some(id));

    post(&app, &format!("{uri}/seal"), &a, json!({ "version": 3 })).await;
    assert_eq!(notes(&app, &b).await.0[0], "trade_sealed");
    post(&app, &format!("{uri}/seal"), &b, json!({ "version": 3 })).await;
    assert_eq!(notes(&app, &a).await.0[0], "trade_done");

    // Read one, then all
    let first = get(&app, "/api/notifications", &b).await.1["items"][0]["id"].as_i64().unwrap();
    let (_, r) = post(&app, "/api/notifications/read", &b, json!({ "ids": [first] })).await;
    assert_eq!(r["unread"], 3);
    assert_eq!(get(&app, "/api/me", &b).await.1["notifications"], 3);
    let (_, r) = post(&app, "/api/notifications/read", &b, json!({})).await;
    assert_eq!(r["unread"], 0);
    // Someone else's notifications stay unread
    assert!(notes(&app, &a).await.1 > 0);
}

#[actix_web::test]
async fn search_finds_cards_sets_and_players() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let ((a_name, a, _), (b_name, b, _)) = pair(&app).await;

    let (_, r) = get(&app, "/api/search?q=MINE", &a).await;
    assert_eq!(r["collections"][0]["slug"], "minerals");
    let (_, r) = get(&app, "/api/search?q=quar", &a).await;
    assert_eq!(r["cards"][0]["slug"], "quartz");
    // French titles, accents ignored
    let req = test::TestRequest::get()
        .uri("/api/search?q=mineraux")
        .cookie(a.clone())
        .cookie(Cookie::new("lang", "fr"))
        .to_request();
    let r = body(test::call_service(&app, req).await).await;
    assert_eq!(r["collections"][0]["title"], "Minéraux");

    let (_, r) = get(&app, &format!("/api/search?q={}", &b_name[..7]), &a).await;
    assert!(r["players"].as_array().unwrap().iter().any(|p| p["username"] == b_name.as_str()));
    let (_, r) = get(&app, &format!("/api/search?q={a_name}"), &a).await;
    assert!(r["players"].as_array().unwrap().iter().all(|p| p["username"] != a_name.as_str()), "not yourself");
    let (_, r) = get(&app, "/api/search?q=q", &b).await;
    assert_eq!(r["cards"], json!([]), "two letters at least");
}

#[actix_web::test]
async fn auth_rate_limits() {
    let Some(ctx) = ctx().await else { return };
    // The real limits, on a state of its own
    let state = web::Data::new(AppState {
        pool: ctx.state.pool.clone(),
        live: Default::default(),
        limits: Default::default(),
        catalog: ctx.state.catalog.clone(),
        config: ctx.state.config.clone(),
    });
    let app = test::init_service(App::new().app_data(state).configure(routes::configure)).await;
    let sign_up = |ip: &str| {
        let name = unique("lim");
        test::TestRequest::post()
            .uri("/api/auth/register")
            .peer_addr(format!("{ip}:4000").parse().unwrap())
            .set_json(json!({ "username": name, "email": format!("{name}@example.com"), "password": "123456789" }))
            .to_request()
    };
    // Sign-ups: five per address, then wait; another address is unaffected
    for _ in 0..5 {
        assert_eq!(test::call_service(&app, sign_up("203.0.113.7")).await.status(), 201);
    }
    assert_eq!(test::call_service(&app, sign_up("203.0.113.7")).await.status(), 429);
    assert_eq!(test::call_service(&app, sign_up("203.0.113.8")).await.status(), 201);

    // Five wrong passwords, then the account waits, whatever the address
    let name = unique("victim");
    let res = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/auth/register")
            .peer_addr("192.0.2.50:4000".parse().unwrap())
            .set_json(json!({ "username": name, "email": format!("{name}@example.com"), "password": "123456789" }))
            .to_request(),
    )
    .await;
    assert_eq!(res.status(), 201);
    let login = |password: &str, ip: &str| {
        test::TestRequest::post()
            .uri("/api/auth/login")
            .peer_addr(format!("{ip}:4000").parse().unwrap())
            .set_json(json!({ "login": name, "password": password }))
            .to_request()
    };
    for _ in 0..5 {
        assert_eq!(test::call_service(&app, login("wrong-password", "198.51.100.1")).await.status(), 422);
    }
    let res = test::call_service(&app, login("123456789", "198.51.100.2")).await;
    assert_eq!(res.status(), 429, "even the right password waits, from any address");
    assert!(res.headers().get("retry-after").is_some());
    let b = body(res).await;
    assert_eq!(b["error"], "too_many_attempts");
    assert!((1..=60).contains(&b["retry_after"].as_u64().unwrap()));
}
