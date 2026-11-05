# AnyCardz

Trading card game: buy boosters, tear them open, fill your binder. Rewrite of the transcendance project with Actix, Postgres and SvelteKit.

- `backend/`: API, Rust, actix-web 4 + Diesel (diesel-async, Postgres)
- `frontend/`: SvelteKit 3 app (adapter-node, SSR)
- Card data lives in a separate repository, [anycardz-data](../anycardz-data), read from `ANYCARDZ_DATA`

Plan, architecture and tickets are in Lific, project `AC` (see AGENTS.md).

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

## API

Everything is under `/api`. Errors are `{"error": "<code>"}` with a stable code that the frontend translates.

| Method | Path                     | Auth | What                                              |
| ------ | ------------------------ | ---- | ------------------------------------------------- |
| POST   | `/auth/register`         |      | `{username, email, password}`, sets the session cookie |
| POST   | `/auth/login`            |      | `{login, password}`, login is a username or email |
| POST   | `/auth/logout`           |      | Ends the current session                          |
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

Texts follow the `lang` cookie (`en` or `fr`), then `Accept-Language`.
