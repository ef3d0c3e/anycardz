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
    };
    let catalog = Catalog::load(&config.data_dir, false).unwrap();
    let state = AppState { pool: db::pool(&url).unwrap(), catalog: Arc::new(catalog), config: Arc::new(config) };
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

#[actix_web::test]
async fn simultaneous_seals_swap_once() {
    let Some(ctx) = ctx().await else { return };
    let app = app!(ctx);
    let ((_, a, a_id), (b_name, b, _)) = pair(&app).await;
    give(&ctx, a_id, "quartz", None, 1).await;
    let id = post(&app, "/api/trades", &a, json!({ "with": b_name })).await.1["id"].as_i64().unwrap();
    let uri = format!("/api/trades/{id}");
    put(&app, &uri, &a, json!({ "version": 1, "give": [{ "card": "quartz" }] })).await;
    let seal = format!("{uri}/seal");
    let both = futures_util::future::join_all([
        post(&app, &seal, &a, json!({ "version": 2 })),
        post(&app, &seal, &b, json!({ "version": 2 })),
    ])
    .await;
    let (ra, rb) = (&both[0], &both[1]);
    assert_eq!((ra.0, rb.0), (200, 200));
    assert_eq!(get(&app, &uri, &a).await.1["state"], "done");
    assert_eq!(owned(&app, &b).await.get("quartz"), Some(&(1, 0)), "exactly one copy moved");
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
