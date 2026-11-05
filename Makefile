# Development commands. Copy .env.example to .env first.
.PHONY: dev db-up db-down api web test test-e2e check fmt prod-up prod-down prod-logs

dev: db-up
	@echo "API on :8080, web on http://localhost:5173"
	@trap 'kill 0' INT TERM; cargo run -p anycardz-api & (cd frontend && npm run dev) & wait

db-up:
	docker compose up -d --wait db

db-down:
	docker compose down

api: db-up
	cargo run -p anycardz-api

web:
	cd frontend && npm run dev

# Backend unit + integration tests (integration needs TEST_DATABASE_URL and the db)
test: db-up
	docker compose exec -T db psql -U anycardz -tc "SELECT 1 FROM pg_database WHERE datname='anycardz_test'" | grep -q 1 \
		|| docker compose exec -T db psql -U anycardz -c "CREATE DATABASE anycardz_test"
	cargo test
	cd frontend && npm run test:unit

# Browser tests (Playwright) against the dev stack; starts the API and web server if needed.
# Uses the system Chromium when there is one, else run `npx playwright install chromium` once.
test-e2e: db-up
	cd frontend && CHROMIUM_PATH="$${CHROMIUM_PATH:-$$(command -v chromium || true)}" npm run test:e2e

check:
	cargo clippy --all-targets -- -D warnings
	cargo fmt --check
	cd frontend && npm run check && npx prettier --check src e2e

fmt:
	cargo fmt
	cd frontend && npx prettier --write src e2e

# Production (compose.prod.yaml); needs .env.prod, see .env.prod.example
# Behind nginx: make prod-up PROD_FILES="-f compose.prod.yaml -f deploy/compose.nginx.yaml"
PROD_FILES ?= -f compose.prod.yaml
PROD = docker compose $(PROD_FILES) --env-file .env.prod

prod-up:
	$(PROD) up -d --build

prod-down:
	$(PROD) down

prod-logs:
	$(PROD) logs -f --tail=100
