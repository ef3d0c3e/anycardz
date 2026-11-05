# AnyCardz

Trading card game: buy boosters, tear them open, fill your binder. Rewrite of the transcendance project with Actix, Postgres and SvelteKit.

- `backend/`: API, Rust, actix-web 4 + Diesel (diesel-async, Postgres)
- `frontend/`: SvelteKit 3 app (adapter-node, SSR)
- Card data lives in a separate repository, [anycardz-data](../anycardz-data), read from `ANYCARDZ_DATA`

Plan, architecture and tickets are in Lific, project `AC`.

## Getting started

Needs Rust (nightly or a recent stable with edition 2024), Node 24, Docker, and the data repository cloned next to this one.

```sh
cp .env.example .env
(cd frontend && npm install)
make dev          # Postgres in Docker, API on :8080, web on http://localhost:5173
```

The API runs its migrations on startup. New accounts start with 1,000 coins.

| Command      | What it does                                            |
| ------------ | ------------------------------------------------------- |
| `make dev`   | Database, API and web dev server together               |
| `make test`  | Backend unit + API integration tests (Postgres), frontend unit tests (Vitest) |
| `make test-e2e` | Browser tests (Playwright) on phone and desktop sizes, against the dev stack |
| `make check` | clippy, rustfmt, svelte-check, prettier                 |
| `make fmt`   | Format Rust and frontend code                           |

CI (`.github/workflows/ci.yml`) runs the same checks on every push: rustfmt, clippy and the API tests against a Postgres service; svelte-check, prettier, Vitest and a production build; then the Playwright suite with the data repository checked out next to the code.

## Deployment

Step by step guide, with Caddy or behind an existing nginx, backups and monitoring: **[docs/deploy.md](docs/deploy.md)**. In short:

`compose.prod.yaml` runs the whole site behind [Caddy](https://caddyserver.com), which gets the HTTPS certificate and is the only container with published ports:

| Service | What it is |
| ------- | ---------- |
| `caddy` | `:80`/`:443`; `/api/*` (WebSocket included) goes to `api`, everything else to `web` |
| `api`   | `backend/Dockerfile`, release build on Debian slim; migrations run on startup |
| `web`   | `frontend/Dockerfile`, the SvelteKit Node server; reaches the API at `http://api:8080` |
| `db`    | Postgres 17, data in the `db-data` volume |

On the server, with Docker, this repository and anycardz-data cloned side by side, and DNS for the domain pointing at it:

```sh
cp .env.prod.example .env.prod   # DOMAIN, POSTGRES_PASSWORD, DATA_DIR, OPERATOR, CONTACT
make prod-up                     # build and start; prod-logs, prod-down
```

Push notifications need a VAPID key pair: run `cargo run -p anycardz-api -- vapid-keys` once and put the private key in `VAPID_PRIVATE_KEY` (`.env.prod`). Without it the site runs with push off and Settings says so. Keep the key: a new one silently ends every device's subscription.

To update, `git pull` both repositories and `make prod-up` again. Card data is read at startup, so a data-only change needs `docker compose -f compose.prod.yaml --env-file .env.prod restart api`.

Cookies are `Secure` and the API believes `X-Forwarded-For` only from Caddy's fixed address (`172.30.0.10`, used for the per-address login limits). With nginx on the host instead, add `-f deploy/compose.nginx.yaml` and use `deploy/nginx.conf`. To try the stack locally over plain HTTP, set `DOMAIN=http://localhost`, `ORIGIN=http://localhost:8088`, `HTTP_PORT=8088` and `SECURE_COOKIES=0`.

## Monitoring

Prometheus and Grafana run elsewhere; the API only exposes metrics. They are served on their own listener, `ANYCARDZ_METRICS_BIND` (`GET /metrics`, OpenMetrics text), never through the public site. When `ANYCARDZ_METRICS_TOKEN` is set, scrapes must send `Authorization: Bearer <token>`. In `compose.prod.yaml` the listener is published on `METRICS_PUBLISH` (default `127.0.0.1:9100`, the host only); if you publish it on all interfaces, set `METRICS_TOKEN`.

- `deploy/prometheus/scrape.yml`: scrape job to add to your Prometheus
- `deploy/grafana/anycardz.json`: dashboard (Dashboards → Import), with traffic, latency, auth, live sockets, economy and DB pool panels
- Logs: `deploy/compose.logs.yaml` adds Loki and Alloy, `deploy/grafana/anycardz-logs.json` is the logs dashboard (see [docs/deploy.md](docs/deploy.md#logs-in-grafana-loki)). `ANYCARDZ_LOG_FORMAT=json` makes the API log one JSON object a line

| Metric | Labels | What it counts |
| ------ | ------ | -------------- |
| `anycardz_http_requests_total` | `method`, `route`, `status` | Requests, by route pattern (unknown paths: `unmatched`) |
| `anycardz_http_request_duration_seconds` | `method`, `route` | Latency histogram (the live socket is left out) |
| `anycardz_auth_attempts_total` | `action` (login, register), `result` (ok, failed, limited) | Logins and sign-ups |
| `anycardz_boosters_bought_total`, `anycardz_boosters_opened_total` | `collection` | Boosters |
| `anycardz_cards_obtained_total` | `source` (pack, tradeup, roulette), `rarity`, `foil` | Cards gained |
| `anycardz_coins_earned_total`, `anycardz_coins_spent_total` | `reason` | Coin flows (signup, streak, sell, topup / booster) |
| `anycardz_roulette_spins_total` | `result` (won, lost) | Foil roulette |
| `anycardz_trades_total` | `result` (done, cancelled) | Trades ended |
| `anycardz_market_listings_total` | `result` (listed, withdrawn, sold) | Marketplace listing events |
| `anycardz_scratch_tickets_total` | `kind` (jackpot, coins, pack, boost, pair, nothing) | Scratch tickets played, by prize |
| `anycardz_missions_done_total` | `kind` (mission key, or bonus) | Daily missions finished |
| `anycardz_push_messages_total` | `result` (sent, gone, failed) | Web Push deliveries; `gone` subscriptions are deleted |
| `anycardz_market_fees_coins_total` | | Coins taken by the 5% sale fee, gone from the economy |
| `anycardz_shouts_total` | | Shoutbox messages |
| `anycardz_live_sockets`, `anycardz_online_users` | | Open live sockets, and players behind them |
| `anycardz_users` | | Registered accounts |
| `anycardz_db_pool_connections`, `_available`, `_waiting` | | Database pool |

## API

Everything is under `/api`. Errors are `{"error": "<code>"}` with a stable code that the frontend translates.

| Method | Path                     | Auth | What                                              |
| ------ | ------------------------ | ---- | ------------------------------------------------- |
| POST   | `/auth/register`         |      | `{username, email, password, terms}` (the current terms version), sets the session cookie |
| POST   | `/auth/login`            |      | `{login, password}`, login is a username or email |
| POST   | `/auth/logout`           |      | Ends the current session                          |
| GET    | `/legal`                 |      | Current terms version and date, operator and contact |
| POST   | `/legal/accept`          | yes  | `{version}`: accepts the current terms (asked when `terms_outdated` on `/me`) |
| POST   | `/account/password`      | yes  | `{current, new}`; signs the other sessions out    |
| POST   | `/account/email`         | yes  | `{password, email}`                               |
| GET    | `/account/sessions`      | yes  | Logged-in devices, with the current one marked    |
| DELETE | `/account/sessions/{id}` | yes  | Signs one device out                              |
| POST   | `/account/sessions/revoke-others` | yes | Signs every other device out             |
| GET    | `/account/export`        | yes  | Everything kept about the player, as a JSON download |
| POST   | `/account/delete`        | yes  | `{password, confirm}` (the username): deletes the account and all its data |
| GET    | `/me`                    | yes  | Profile, coins, pile, daily streak                |
| GET    | `/collections`           |      | Collections with their featured card              |
| GET    | `/collections/{slug}`    |      | A collection with all its cards                   |
| GET    | `/cards/{slug}`          |      | One card                                          |
| GET    | `/cards/{slug}/art?w=`   |      | Artwork thumbnail, `w` is 420 or 840              |
| GET    | `/assets/{name}`         |      | Foil masks and the wrapper texture                |
| POST   | `/shop/boosters`         | yes  | `{collection}`, buys a booster into the pile      |
| GET    | `/shop/topups`           |      | Coin packs, `checkout: false` while no payment provider exists |
| POST   | `/shop/topup`            | yes  | `{pack}`, grants the coins at once (demo, logged in `topups`) |
| POST   | `/pile/open`             | yes  | `{collection?}`, opens a booster: 5 cards         |
| POST   | `/streak/claim`          | yes  | Daily bonus                                       |
| POST   | `/free-packs/claim`      | yes  | 3 free packs (pile entry `*`, drawn from every collection) every 2 h; `/me.free_packs` says when |
| POST   | `/tickets/scratch`       | yes  | Plays a scratch ticket (3 at most, one back every 20 min; `/me.tickets`): `{slots, prize, coins_won, coins, pile, tickets}`. Three gems 1000 coins (0.5%), coins 250 (4%), packs a free pack (3%), sparkles the foil roulette ×2 for 2 h, capped at 100% (2.5%); any pair 20 coins (30%) |
| GET    | `/missions`              | yes  | Today's 3 missions (UTC day) with progress and rewards, and whether the all-three bonus pack was given; also in `/me` |
| GET    | `/achievements`          | yes  | Every achievement with the player's progress and when it was earned |
| GET    | `/leaderboards?board=&period=` | yes | `board`: sets, achievements, legendaries, foils, trades, streak; `period`: week or all (streak: all). Top 20 and the caller's rank |
| GET    | `/push` | yes | `{key, subscribed}`: the VAPID public key (null when push is off) and whether this device is subscribed |
| PUT    | `/push` | yes | `PushSubscription.toJSON()`: subscribes this device (one per session, removed when it signs out). Known push services only |
| DELETE | `/push` | yes | Unsubscribes this device |
| GET    | `/market?q=&card=&collection=&rarity=&foil=&sort=&page=` | yes | Open listings by other players (blocked ones hidden), cheapest first or `sort=new`, 24 a page |
| GET    | `/market/mine` | yes | The caller's open listings (20 at most) and last 20 sales |
| POST   | `/market` | yes | `{card, foil, price}`: lists one copy, which leaves the binder. Price from the shop sell price to 10× it |
| DELETE | `/market/{id}` | yes | Withdraws a listing: the copy comes back as it was |
| POST   | `/market/{id}/buy` | yes | Buys at the listed price; 5% fee (rounded up) is burned; 3 purchases a day from the same seller |
| GET    | `/binder`                | yes  | Owned cards and completion per collection         |
| POST   | `/binder/sell`           | yes  | `{card, foil}`, sells one copy                    |
| GET    | `/binder?details=1`      | yes  | Same, plus each owned card's details              |
| POST   | `/forge/tradeup`         | yes  | `{cards: [{card, foil, count}]}`, 10 of one rarity into 1 of the next |
| POST   | `/forge/roulette`        | yes  | `{target, burn: [...]}`: win turns the copy foil, a loss destroys it |
| GET    | `/players?q=`            | yes  | Players whose name contains `q`                   |
| GET    | `/players/{name}/cards`  | yes  | A player's cards, to ask for in a trade           |
| POST   | `/trades`                | yes  | `{with}`, opens a trade (or returns the open one) |
| GET    | `/trades`, `/trades/{id}`| yes  | Trades, from the caller's side (`mine` / `theirs`) |
| PUT    | `/trades/{id}`           | yes  | `{version, give, get, give_coins, get_coins}`, replaces the offer, clears both seals |
| POST   | `/trades/{id}/seal`      | yes  | `{version}`; the second seal on a version swaps   |
| POST   | `/trades/{id}/cancel`    | yes  | Cancels an open trade                             |
| GET    | `/live` (WebSocket)      | yes  | Push only: a `hello` snapshot, then `pull`, `chat`, `trade` and `notification` events |
| GET    | `/shoutbox`              | yes  | The last 50 messages, without blocked players     |
| POST   | `/shoutbox`              | yes  | `{body, card?: {card, foil}}`, a brag needs the card; 1 message per 2 s |
| GET    | `/cards?slugs=a,b`       |      | Several cards at once                             |
| GET    | `/players/{name}`        | yes  | Profile: bio, avatar, stats, relation to you      |
| PUT    | `/me/profile`            | yes  | `{bio}`                                           |
| PUT    | `/me/avatar`             | yes  | Raw image body, cropped to a 256 px square        |
| GET    | `/avatars/{name}`        |      | The avatar JPEG                                   |
| GET    | `/friends`               | yes  | Friends (with online state), incoming and outgoing requests |
| POST / DELETE | `/friends/{name}` | yes  | Ask (or accept) / remove, decline or cancel       |
| GET    | `/blocks`, POST / DELETE `/blocks/{name}` | yes | Block list, block, unblock     |
| GET    | `/notifications`         | yes  | The last 50, with the unread count                |
| POST   | `/notifications/read`    | yes  | `{ids?}`: those, or all                           |
| GET    | `/search?q=`             | yes  | Cards, sets and players; accents and case ignored |

Texts follow the `lang` cookie (`en` or `fr`), then `Accept-Language`.

## License

AnyCardz is free software under the [GNU Affero General Public License v3.0](LICENSE) or later. If you run a modified version for others, the AGPL asks you to offer them its source: the site links to it from the account menu and the sign-in pages, set in `frontend/src/lib/source.ts`.

The card data, [anycardz-data](https://github.com/ef3d0c3e/anycardz-data), is a separate work under [CC BY-NC-SA 3.0](https://creativecommons.org/licenses/by-nc-sa/3.0/). All of its texts and images come from Wikipedia and Wikimedia Commons; each item also keeps the licence and attribution of its source page.
