# Deploying AnyCardz

The whole site runs in Docker: Postgres, the API, the web server, and (by
default) Caddy for HTTPS. This guide covers a server that already has nginx in
front of everything, then monitoring with Prometheus and Grafana.

- [What you need](#what-you-need)
- [1. Get the code and the card data](#1-get-the-code-and-the-card-data)
- [2. Configure](#2-configure)
- [3a. Start, with Caddy (nothing else on 80/443)](#3a-start-with-caddy-nothing-else-on-80443)
- [3b. Start, behind nginx](#3b-start-behind-nginx)
- [4. Check it works](#4-check-it-works)
- [Updating, backups, logs](#updating-backups-logs)
- [Monitoring with Prometheus and Grafana](#monitoring-with-prometheus-and-grafana)
- [Logs in Grafana (Loki)](#logs-in-grafana-loki)

## What you need

- A Linux server with Docker Engine and the Compose plugin (`docker compose version`)
- A domain name whose A/AAAA records point at the server
- 2 GB of RAM to build the images (the Rust build is the heaviest step), 1 GB to run them
- For [3b](#3b-start-behind-nginx): nginx and certbot on the host

## 1. Get the code and the card data

The API reads the cards from a checkout of anycardz-data. Clone both side by side:

```sh
git clone https://github.com/ef3d0c3e/anycardz.git
git clone https://github.com/ef3d0c3e/anycardz-data.git
cd anycardz
```

## 2. Configure

```sh
cp .env.prod.example .env.prod
chmod 600 .env.prod
```

| Variable | What to put |
| -------- | ----------- |
| `DOMAIN` | The public host name, e.g. `anycardz.example.org` |
| `POSTGRES_PASSWORD` | A long random string (`openssl rand -hex 24`) |
| `OPERATOR`, `CONTACT` | Who runs the site and an address players can write to. Shown in the terms and privacy notice |
| `DATA_DIR` | Path to the anycardz-data checkout, relative to the compose file: `../anycardz-data` |
| `VAPID_PRIVATE_KEY` | Web Push key, see below. Leave empty to run without push notifications |
| `VAPID_SUBJECT` | Optional; defaults to `mailto:` + `CONTACT` when that is an email address |
| `METRICS_PUBLISH`, `METRICS_TOKEN` | Where Prometheus scrapes, see [Monitoring](#monitoring-with-prometheus-and-grafana) |
| `RUST_LOG` | `info` (default), `debug` to investigate |

Push notifications need a key pair, made once. Keep it: a new key silently ends
every device's subscription.

```sh
docker compose -f compose.prod.yaml --env-file .env.prod build api
docker compose -f compose.prod.yaml --env-file .env.prod run --rm --no-deps api anycardz-api vapid-keys
# copy the ANYCARDZ_VAPID_PRIVATE_KEY=… value into VAPID_PRIVATE_KEY in .env.prod
```

## 3a. Start, with Caddy (nothing else on 80/443)

Caddy gets the certificate from Let's Encrypt by itself:

```sh
make prod-up        # docker compose -f compose.prod.yaml --env-file .env.prod up -d --build
```

That is all; go to [4](#4-check-it-works).

## 3b. Start, behind nginx

When nginx already serves other sites on 80/443, leave Caddy out with the
override `deploy/compose.nginx.yaml`. It publishes the API and the site on the
host's loopback only (`127.0.0.1:8080` and `127.0.0.1:3000`), and tells the API
to believe nginx's `X-Forwarded-For`.

**Start the containers**

```sh
docker compose -f compose.prod.yaml -f deploy/compose.nginx.yaml --env-file .env.prod up -d --build
```

To type plain `docker compose …` on this server without the `-f` options (and
never start the development `compose.yaml` by mistake), make `.env` point to
`.env.prod` and name the files there:

```sh
echo 'COMPOSE_FILE=compose.prod.yaml:deploy/compose.nginx.yaml' >> .env.prod
ln -s .env.prod .env
docker compose up -d --build      # now the same as the long command above
```

Ports 8080 or 3000 taken on the host? Set `API_PORT` / `WEB_PORT` in `.env.prod`
and change the two `upstream` lines in the nginx file to match.

**Get a certificate**

```sh
sudo certbot certonly --webroot -w /var/www/html -d anycardz.example.org
# or, if nginx already serves the name on port 80: sudo certbot certonly --nginx -d anycardz.example.org
```

**Add the site to nginx**

```sh
sudo cp deploy/nginx.conf /etc/nginx/sites-available/anycardz
sudo sed -i 's/anycardz.example.org/YOUR.DOMAIN/g' /etc/nginx/sites-available/anycardz
sudo ln -s /etc/nginx/sites-available/anycardz /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl reload nginx
```

(On distributions without `sites-available`, copy it to `/etc/nginx/conf.d/anycardz.conf`.)

What the file does:

- `/api/` goes to the API, with the WebSocket upgrade the live feed needs and a
  long read timeout so the socket stays open
- everything else goes to the SvelteKit server
- avatars up to 4 MB (`client_max_body_size 5m`), gzip for text, a week of
  browser caching for the favicon, icons and manifest

**Client addresses.** Login and sign-up limits are per client address, so the
API must see the players' addresses, not nginx's. Requests from the host reach
the containers from the Docker network's gateway, `172.30.0.1`, which is what
the override trusts; nginx then passes the real address in `X-Forwarded-For`.
Check it once the site is up: try a wrong password from your browser, then look
at the address the API counted:

```sh
docker compose -f compose.prod.yaml -f deploy/compose.nginx.yaml --env-file .env.prod logs api | grep "login failed"
# … login failed client=203.0.113.7   ← your public address: good
```

- `client=172.30.0.1` on every line: your Docker setup forwards from another
  address. Put the address you see in `TRUSTED_PROXIES` in `.env.prod` and run
  the `up -d` command again.
- An address a client could choose (e.g. one sent in their own
  `X-Forwarded-For`): nginx is not setting the header. Every `location` in the
  file sets its `proxy_set_header` lines on purpose, keep it that way when you
  edit it (nginx drops server-level ones in a location that has its own).

The API ports are bound to `127.0.0.1`, so only processes on the server itself
can talk to them directly.

## 4. Check it works

```sh
curl -s https://YOUR.DOMAIN/api/health   # {"ok":true}
```

Then open the site, create an account, open a pack, and check the live feed on
the side (that is the WebSocket working through the proxy). Settings should say
"Off for this device" under notifications when the VAPID key is set, or
"not set up on this server" otherwise.

## Updating, backups, logs

The commands below use the Caddy setup; behind nginx, add
`-f deploy/compose.nginx.yaml` after `-f compose.prod.yaml`.

```sh
# Update: migrations run when the API starts
git pull && (cd ../anycardz-data && git pull)
make prod-up

# New card data only: the API reads it at startup
docker compose -f compose.prod.yaml --env-file .env.prod restart api

# Backup the database (players, cards, trades...)
docker compose -f compose.prod.yaml --env-file .env.prod exec -T db \
  pg_dump -U anycardz -Fc anycardz > anycardz-$(date +%F).dump

# Restore into a fresh database
docker compose -f compose.prod.yaml --env-file .env.prod exec -T db \
  pg_restore -U anycardz -d anycardz --clean --if-exists < anycardz-2026-11-05.dump

# Logs
make prod-logs
```

Thumbnails live in the `cache` volume and are rebuilt on demand, so they need no
backup. Keep `.env.prod` somewhere safe: it holds the database password and the
push key.

## Monitoring with Prometheus and Grafana

The API serves Prometheus metrics on a listener of its own (port 9100 in the
container), never through the public site. `compose.prod.yaml` publishes it on
`METRICS_PUBLISH`.

### Prometheus on the same machine

Keep the default `METRICS_PUBLISH=127.0.0.1:9100`, and add the job from
`deploy/prometheus/scrape.yml` to `prometheus.yml`, targeting `127.0.0.1:9100`
(drop the `authorization` block when no token is set):

```yaml
scrape_configs:
  - job_name: anycardz
    scrape_interval: 30s
    static_configs:
      - targets: ['127.0.0.1:9100']
```

Prometheus in a container on the same host reaches it with `network_mode: host`,
or use `host.docker.internal:9100` with `METRICS_PUBLISH=172.17.0.1:9100`.

### Prometheus on another machine

Publish on all interfaces, require a token, and only let the Prometheus server
through the firewall:

```sh
# .env.prod
METRICS_PUBLISH=0.0.0.0:9100
METRICS_TOKEN=<openssl rand -hex 32>

sudo ufw allow from <prometheus-ip> to any port 9100 proto tcp
```

On the Prometheus side, put the token in a file and use the job in
`deploy/prometheus/scrape.yml` as is:

```yaml
scrape_configs:
  - job_name: anycardz
    scrape_interval: 30s
    authorization:
      type: Bearer
      credentials_file: /etc/prometheus/anycardz.token
    static_configs:
      - targets: ['anycardz.example.org:9100']
```

Check from the Prometheus server:

```sh
curl -s -H "Authorization: Bearer $(cat /etc/prometheus/anycardz.token)" http://anycardz.example.org:9100/metrics | head
```

and in Prometheus, Status → Targets should show the `anycardz` job up.

### The Grafana dashboard

`deploy/grafana/anycardz.json` is a ready dashboard:

- **Overview:** online players, accounts, sign-ups, boosters opened
- **Traffic:** requests, statuses, latency percentiles, slowest routes, 5xx errors
- **Auth and live:** logins, sign-ups, live sockets
- **Game:** boosters, cards by rarity and source, coins in and out, roulette, trades, shoutbox, foil share
- **Progression and marketplace:** achievements, daily missions, free packs, market listings, fees, push deliveries
- **Database:** connection pool

To import it:

1. In Grafana, Connections → Data sources: add the Prometheus that scrapes AnyCardz, if not there yet.
2. Dashboards → New → Import, upload `deploy/grafana/anycardz.json`, Import.
3. Pick the data source in the **Prometheus** menu at the top of the dashboard.
   The **Job** menu lists every Prometheus job exposing AnyCardz metrics (All by default).

Or with Grafana's API (a service account token with the Editor role):

```sh
jq '{dashboard: ., overwrite: true}' deploy/grafana/anycardz.json |
  curl -s -X POST -H "Authorization: Bearer $GRAFANA_TOKEN" -H 'content-type: application/json' \
    --data @- https://grafana.example.org/api/dashboards/db
```

Or provision it with Grafana's files, e.g. `/etc/grafana/provisioning/dashboards/anycardz.yaml`
with the JSON in `/var/lib/grafana/dashboards/`:

```yaml
apiVersion: 1
providers:
  - name: anycardz
    type: file
    options:
      path: /var/lib/grafana/dashboards
```

Worth an alert: `sum(rate(anycardz_http_requests_total{status=~"5.."}[5m])) > 0.1`
(server errors), `anycardz_db_pool_waiting > 0` for 5 minutes (database
saturated), and the target being down (`up{job="anycardz"} == 0`).

## Logs in Grafana (Loki)

`docker compose logs` works on the server, but searching weeks of logs from
Grafana is nicer. [Loki](https://grafana.com/oss/loki/) stores logs (it only
indexes a few labels, so it stays small and light), and
[Alloy](https://grafana.com/oss/alloy/) collects the containers' output and
sends it to Loki. Both run next to the app with one more override,
`deploy/compose.logs.yaml`:

```sh
# Caddy setup
docker compose -f compose.prod.yaml -f deploy/compose.logs.yaml --env-file .env.prod up -d
# nginx setup
docker compose -f compose.prod.yaml -f deploy/compose.nginx.yaml -f deploy/compose.logs.yaml --env-file .env.prod up -d
```

What it does:

- the API switches to JSON logs (`ANYCARDZ_LOG_FORMAT=json`): its level, route,
  status and client address become searchable
- Alloy reads the logs of this compose project's containers (api, web, db,
  caddy) and labels each line with `app="anycardz"` and its `service`
- Loki keeps them 30 days on the `loki-data` volume (`retention_period` in
  `deploy/loki/loki.yaml`) and listens on `LOKI_PUBLISH`, by default
  `127.0.0.1:3100`: the server only

Alloy reads the Docker socket to find the containers. That gives it a lot of
power over the host, like any tool that reads Docker logs: keep the image
pinned and the config as it is.

### Grafana on the same machine

Add a Loki data source with the URL `http://127.0.0.1:3100` (from a Grafana
container: `network_mode: host`, or publish Loki on the Docker bridge as for
Prometheus).

### Grafana on another machine

Loki has no login of its own, so never publish it on the internet as is. Either
reach it over a private network (WireGuard, Tailscale, a VPC) with
`LOKI_PUBLISH=<private-ip>:3100`, or put it behind nginx with HTTPS and a
password:

```sh
sudo htpasswd -c /etc/nginx/loki.htpasswd grafana   # apache2-utils
```

```nginx
server {
    listen 443 ssl;
    http2 on;
    server_name loki.example.org;
    ssl_certificate     /etc/letsencrypt/live/loki.example.org/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/loki.example.org/privkey.pem;

    location / {
        auth_basic           "Loki";
        auth_basic_user_file /etc/nginx/loki.htpasswd;
        proxy_pass           http://127.0.0.1:3100;
        proxy_read_timeout   5m;
    }
}
```

In Grafana: Connections → Data sources → Loki, URL `https://loki.example.org`,
Authentication → Basic authentication with that user and password.

### The logs dashboard

Import `deploy/grafana/anycardz-logs.json` like the metrics dashboard, then pick
the Loki data source in the **Loki** menu at the top. It shows:

- log lines per service over time
- API warnings and errors, as a graph and as a list (4xx responses, which are
  player mistakes like a wrong password or too few coins, are left out)
- every log line, narrowed with the **Service** and **Contains** menus; API lines
  read as "message · method route status", a click shows all their fields
- failed and rate-limited logins with the client address the limits counted

For anything else, Explore → Loki. Some queries:

```logql
{app="anycardz", service="api", level="ERROR"}                       # server errors
{app="anycardz", service="api"} | json | span_http_status_code >= 500  # 5xx responses
{app="anycardz", service="api"} | json | span_http_route="/api/market/{id}/buy"
{app="anycardz", service="api"} |= "login failed" | json | fields_client="203.0.113.7"
{app="anycardz", service=~"web|caddy"} |= "error"
```

An alert worth having, in Grafana Alerting with the Loki data source:
`sum(count_over_time({app="anycardz", service="api", level="ERROR"} [5m])) > 0`.

