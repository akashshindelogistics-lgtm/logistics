# Logistics System

[![Frontend Unit Tests (Vitest)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/frontend-unit-tests.yml/badge.svg)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/frontend-unit-tests.yml)

[![Frontend Integration Tests (Playwright)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/frontend-integration.yml/badge.svg)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/frontend-integration.yml)

[![Periodic Cargo Tests](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/periodic-tests.yml/badge.svg)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/periodic-tests.yml)

[![Pages — Deploy Frontend + Swagger UI](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/pages.yml/badge.svg)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/pages.yml)

[![Deploy Backend](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/deploy-backend.yml/badge.svg)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/deploy-backend.yml)

A logistics management platform for tracking organizations, their vehicle
fleets, stock, customers, and delivery dispatches. It ships as a Rust REST
API backed by MySQL, with a React + TypeScript dashboard for day-to-day
operations, live location maps, and AI-generated dispatch summaries.

📖 **Live dashboard:** <https://akashshindelogistics-lgtm.github.io/logistics/>
&nbsp;·&nbsp; **API docs (Swagger UI):** <https://akashshindelogistics-lgtm.github.io/logistics/api-docs/>

## Features

- **Organizations** — create and manage organizations, each with its own
  location, vehicles, stock, and customers.
- **Vehicles** — register vehicles per organization and track their live
  location.
- **Stock** — add, update, and remove stock items held by an organization.
- **Customers** — manage customer records and their delivery locations.
- **Dispatches** — create dispatch orders that move stock from an
  organization to a customer, and advance each one through a lifecycle
  (`PENDING → CONFIRMED → LOADED → IN_TRANSIT → DELIVERED`/`RETURNED`/
  `CANCELLED`) with a full timestamped status history and proof of
  delivery (receiver name plus a signature/photo) required to mark one
  `DELIVERED`.
- **AI dispatch summaries** — generate a natural-language summary of a
  dispatch's status using the Anthropic (Claude) API.
- **Authentication** — org-level login secured with JWTs and bcrypt-hashed
  passwords.
- **Interactive API docs** — a Swagger UI generated from the API with
  `utoipa`, auto-deployed to
  [GitHub Pages](https://akashshindelogistics-lgtm.github.io/logistics/api-docs/)
  on every push to `main`/`master`.
- **Web dashboard** — a React frontend for organizations, vehicles, stock,
  customers, and dispatches, including a live Leaflet map of locations,
  deployed alongside the docs on GitHub Pages.

## Tech stack

**Backend**
- [Rust](https://www.rust-lang.org/) (2024 edition)
- [Actix Web](https://actix.rs/) — HTTP server and routing
- [SQLx](https://github.com/launchbadge/sqlx) / [MySQL](https://www.mysql.com/) — data storage
- [utoipa](https://github.com/juhaku/utoipa) + [Swagger UI](https://swagger.io/tools/swagger-ui/) — OpenAPI spec and docs
- [jsonwebtoken](https://github.com/Keats/jsonwebtoken) + [bcrypt](https://github.com/Keats/rust-bcrypt) — auth
- [reqwest](https://github.com/seanmonstar/reqwest) — outbound HTTP calls to the Anthropic API

**Frontend** (`frontend/`)
- [React 19](https://react.dev/) + [TypeScript](https://www.typescriptlang.org/)
- [Vite](https://vitejs.dev/) — dev server and build tooling
- [React Router](https://reactrouter.com/) — client-side routing
- [Leaflet](https://leafletjs.com/) / [React Leaflet](https://react-leaflet.js.org/) — maps
- [Axios](https://axios-http.com/) — API client
- [Playwright](https://playwright.dev/) — end-to-end tests

## Project structure

```
.
├── src/
│   ├── logistics/
│   │   ├── ai/          # Claude-powered dispatch summaries
│   │   ├── auth/        # JWT auth and org credentials
│   │   ├── customer/    # Customer domain model
│   │   ├── db/          # Database connection setup
│   │   ├── dispatch/    # Dispatch order domain model
│   │   ├── orgs/        # Organization domain model
│   │   ├── server/      # Actix routes and API wiring
│   │   ├── stock/       # Stock domain model
│   │   └── vehicle/     # Vehicle domain model
│   ├── bin/gen_openapi.rs  # Generates the OpenAPI spec for Swagger UI
│   └── main.rs
└── frontend/
    ├── src/
    │   ├── api/         # Axios clients per resource
    │   ├── components/  # Shared UI (navbar, sidebar, map, icons)
    │   └── pages/        # Dashboard, orgs, vehicles, customers, dispatches, auth
    └── tests/           # Playwright end-to-end tests
```

## Getting started

### Prerequisites

- Rust (stable, 2024 edition support) and Cargo
- MySQL 8.x
- Node.js 18+ and npm

### Backend

The server currently connects to a local MySQL instance with fixed
credentials (`root` / `password` on `localhost:3306`), creating the
`logistics` database automatically if it doesn't exist yet:

```bash
# Start MySQL locally, e.g. via Docker
docker run -d --name logistics-mysql -p 3306:3306 \
  -e MYSQL_ROOT_PASSWORD=password -e MYSQL_DATABASE=logistics mysql:8.0

# Optional: enables AI-generated dispatch summaries
export ANTHROPIC_API_KEY="sk-ant-..."

cargo run
```

The API server starts at `http://127.0.0.1:8080`.

To regenerate the OpenAPI spec used by Swagger UI:

```bash
cargo run --bin gen_openapi
```

### Frontend

```bash
cd frontend
npm install
npm run dev
```

The dashboard runs on Vite's default dev server port and talks to the API
at `http://127.0.0.1:8080`.

### Tests

```bash
# Rust unit tests
cargo test

cd frontend

# Frontend unit tests (Vitest)
npm run test:unit

# Frontend end-to-end tests (Playwright)
npm run test:e2e
```

## API overview

Key REST endpoints exposed by the server (see the
[published Swagger UI](https://akashshindelogistics-lgtm.github.io/logistics/api-docs/)
for the full spec, request/response schemas, and try-it-out support):

All routes are served under the `/api` prefix.

| Method | Path | Description |
|---|---|---|
| POST | `/api/auth/login` | Log in to an organization |
| GET | `/api/auth/me` | Get the authenticated organization |
| GET | `/api/auth/orgs` | List organizations available for login |
| GET/POST | `/api/orgs` | List / create organizations |
| GET/PUT/DELETE | `/api/orgs/{id}` | Get, update, or delete an organization |
| PUT | `/api/orgs/{id}/location` | Update an organization's location |
| GET/POST | `/api/orgs/{id}/godowns` | List / create an organization's godowns (warehouses) |
| GET/PUT/DELETE | `/api/godowns/{gid}` | Get, rename/re-address, or delete a godown |
| PUT | `/api/godowns/{gid}/location` | Update a godown's location |
| POST/PUT/DELETE | `/api/godowns/{gid}/stock`, `/api/godowns/{gid}/stock/{desc}` | Add, update, or remove a godown's stock |
| POST | `/api/orgs/{id}/vehicles` | Add a vehicle to an organization |
| GET/DELETE | `/api/vehicles`, `/api/vehicles/{reg}` | List vehicles / remove one |
| PUT | `/api/vehicles/{reg}/location` | Update a vehicle's location |
| GET/POST | `/api/customers` | List / create customers |
| PUT | `/api/customers/{id}/location` | Update a customer's location |
| POST | `/api/orgs/{id}/dispatch` | Dispatch stock from an org to a customer |
| GET | `/api/dispatches` | List dispatch orders |
| PUT | `/api/dispatches/{id}/status` | Advance a dispatch's lifecycle status |
| GET | `/api/dispatches/{id}/summary` | AI-generated summary of a dispatch |
| GET | `/api/health` | Health check |

The published Swagger UI (see CI/CD below) has "Try it out" enabled and the
server enables permissive CORS, so it can call the deployed API directly.
Run the backend locally with `cargo run` to exercise it against a local
instance instead.

## Deployment

Free-tier hosting: the frontend + Swagger UI on **GitHub Pages**, the Rust
API + MySQL on an **Oracle Cloud "Always Free"** VM (`docker compose` —
Caddy for auto-HTTPS, plus a DuckDNS updater), both rolled out by GitHub
Actions. Full runbook in [`deploy/README.md`](deploy/README.md).

## CI/CD

GitHub Actions workflows in `.github/workflows/`:

- **pages.yml** — builds the React dashboard, validates the OpenAPI spec,
  and deploys both to
  [GitHub Pages](https://akashshindelogistics-lgtm.github.io/logistics/)
  (app at `/`, Swagger UI at `/api-docs/`) on push to `main`/`master`.
- **deploy-backend.yml** — builds the API container, pushes it to GHCR, and
  SSHes into the Oracle VM to roll it out with `docker compose`.
- **swagger-pages.yml** — validates the OpenAPI spec and deploys the
  Swagger UI to
  [GitHub Pages](https://akashshindelogistics-lgtm.github.io/logistics/) on
  push to `main`/`master`.
- **frontend-unit-tests.yml** — type-checks the frontend and runs its
  Vitest unit suite. No backend or database needed, so it's the fastest
  signal on a frontend change.
- **frontend-integration.yml** — runs the Playwright end-to-end suite
  against a MySQL service container.
- **periodic-tests.yml** — runs the Cargo test suite on a schedule and on
  every push/PR.

## Contributing

Issues and pull requests are welcome. Please open an issue to discuss any
significant change before submitting a PR.

## License

This project is licensed under the [Apache License 2.0](LICENSE).
