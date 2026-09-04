# Logistics System

[![Frontend Unit Tests (Vitest)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/frontend-unit-tests.yml/badge.svg)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/frontend-unit-tests.yml)

[![Frontend Integration Tests (Playwright)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/frontend-integration.yml/badge.svg)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/frontend-integration.yml)

[![Periodic Cargo Tests](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/periodic-tests.yml/badge.svg)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/periodic-tests.yml)

[![Pages — Deploy Frontend + Swagger UI](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/pages.yml/badge.svg)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/pages.yml)

[![Release](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/release.yml/badge.svg)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/release.yml)
&nbsp; [![Latest release](https://img.shields.io/github/v/release/akashshindelogistics-lgtm/logistics?sort=semver)](https://github.com/akashshindelogistics-lgtm/logistics/releases)

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
  location. Record each vehicle's compliance paperwork (insurance, RC,
  permit, PUC and fitness certificate) with expiry dates; the dashboard
  flags documents that are expiring within 30 days or already expired.
- **Drivers** — keep driver records (name, licence number, phone) per
  organization and assign one to a vehicle from the dashboard. A vehicle
  needs an **active** assigned driver, spare capacity, and no trip already
  in progress before it can be selected for a dispatch.
- **Stock** — add, update, and remove stock items held in an organization's
  godowns, and transfer a stock item between two godowns with a recorded
  audit trail of every move.
- **Customers** — manage customer records and their delivery locations. Each
  customer belongs to one organization and is never shared between orgs.
- **Dispatches** — create dispatch orders that move one or more stock line
  items from an organization to a customer, and advance each one through a
  lifecycle (`PENDING → CONFIRMED → LOADED → IN_TRANSIT → DELIVERED`/
  `RETURNED`/`CANCELLED`) with a full timestamped status history. Marking one
  `DELIVERED` requires proof of delivery (receiver name plus a
  signature/photo); marking one `RETURNED` credits the shipment's stock back
  into a godown.
- **Freight billing** — raise one invoice per dispatch with an amount and a
  due date; the dashboard tracks each invoice as paid / pending / overdue
  and rolls a customer's unpaid invoices up into an outstanding balance.
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
│   │   ├── billing/     # Freight invoices
│   │   ├── customer/    # Customer domain model
│   │   ├── db/          # Database connection setup
│   │   ├── dispatch/    # Dispatch order domain model
│   │   ├── driver/      # Driver domain model
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

By default the server connects to a local MySQL instance (`root` / `password`
on `localhost:3306`), creating the `logistics` database automatically if it
doesn't exist yet. Connection settings can be overridden with `DATABASE_URL`
or the `MYSQL_*` variables (see `src/logistics/db/connection.rs`).

```bash
# Start MySQL locally, e.g. via Docker
docker run -d --name logistics-mysql -p 3306:3306 \
  -e MYSQL_ROOT_PASSWORD=password -e MYSQL_DATABASE=logistics mysql:8.0

# Optional: enables AI-generated dispatch summaries
export ANTHROPIC_API_KEY="sk-ant-..."
# Only needed if the key above is identity-linked (not workspace-scoped):
# the workspace to bill the request to, e.g. wrkspc_01ABC...
export ANTHROPIC_WORKSPACE_ID="wrkspc_..."

cargo run
```

The API server starts at `http://127.0.0.1:8080`.

> **Auth secret:** `debug` builds (`cargo run`, `cargo test`) use a built-in
> development JWT secret. A **release build refuses to start** unless
> `JWT_SECRET` is set to a random string of 32+ characters
> (`openssl rand -base64 48`).

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

# Watch the whole product run in a real browser window: register, login,
# godowns/stock/transfer, fleet + compliance, driver, customer, a dispatch
# carried through delivery, and entity edits. Starts the Vite dev server
# and the Rust API itself if they aren't already running.
npm run test:e2e:demo
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
| POST | `/api/godowns/{gid}/transfer` | Move a stock item from this godown to another godown of the same org |
| GET | `/api/orgs/{id}/stock-transfers` | Godown-to-godown transfer history (audit trail) |
| POST | `/api/orgs/{id}/vehicles` | Add a vehicle to an organization |
| GET/DELETE | `/api/vehicles`, `/api/vehicles/{reg}` | List vehicles / remove one |
| PUT | `/api/vehicles/{reg}` | Update a vehicle's capacity and unit |
| PUT | `/api/vehicles/{reg}/location` | Update a vehicle's location |
| PUT | `/api/vehicles/{reg}/driver` | Assign (or clear) the vehicle's driver |
| GET/POST | `/api/drivers`, `/api/orgs/{id}/drivers` | List / add drivers |
| PUT/DELETE | `/api/drivers/{id}` | Update (incl. active flag) or remove a driver |
| GET/POST | `/api/vehicles/{reg}/documents` | List / record a vehicle's compliance paperwork (insurance, RC, permit, PUC, fitness) |
| PUT/DELETE | `/api/vehicle-documents/{id}` | Renew (update) or delete a compliance document |
| GET | `/api/orgs/{id}/vehicle-documents` | Whole-fleet compliance list, soonest expiry first |
| GET/POST | `/api/customers`, `/api/orgs/{id}/customers` | List the org's customers / add one |
| PUT/DELETE | `/api/customers/{id}/location`, `/api/customers/{id}` | Update a customer's location / delete the customer |
| POST | `/api/orgs/{id}/dispatch` | Dispatch stock from an org to one of its customers |
| GET | `/api/dispatches` | List dispatch orders |
| PUT | `/api/dispatches/{id}/status` | Advance a dispatch's lifecycle status |
| GET | `/api/dispatches/{id}/summary` | AI-generated summary of a dispatch |
| GET/POST | `/api/dispatches/{id}/invoice` | Get / raise the freight invoice for a dispatch |
| PUT | `/api/invoices/{id}` | Amend an unpaid invoice's amount or due date |
| POST | `/api/invoices/{id}/pay` | Mark an invoice paid |
| GET | `/api/orgs/{id}/invoices` | All freight invoices for the org |
| GET | `/api/customers/{id}/billing` | A customer's payment standing (outstanding, overdue) |
| GET | `/api/health` | Health check |

The published Swagger UI (see CI/CD below) has "Try it out" enabled and the
server enables permissive CORS, so it can call the deployed API directly.
Run the backend locally with `cargo run` to exercise it against a local
instance instead.

## Deployment

Free-tier hosting: the frontend + Swagger UI on **GitHub Pages** (the live
demo, tracking `master`), the Rust API + MySQL on an **Oracle Cloud "Always
Free"** VM (`docker compose` — Caddy for auto-HTTPS, plus a DuckDNS updater).
Full runbook in [`deploy/README.md`](deploy/README.md).

## Releases

Backend and frontend are versioned together (SemVer). `release-please` keeps a
rolling release PR from the conventional-commit history; merging it tags
`vX.Y.Z`, which drives `release.yml` to publish the GHCR image
(`logistics-api:X.Y.Z`), a `frontend-X.Y.Z.tar.gz` bundle, the OpenAPI spec,
and a GitHub Release — optionally rolling the Oracle VM to that version. See
[`docs/releasing.md`](docs/releasing.md) and the
[Releases page](https://github.com/akashshindelogistics-lgtm/logistics/releases).

## CI/CD

GitHub Actions workflows in `.github/workflows/`:

- **pages.yml** — builds the React dashboard, validates the OpenAPI spec, and
  deploys both to
  [GitHub Pages](https://akashshindelogistics-lgtm.github.io/logistics/)
  (app at `/`, Swagger UI at `/api-docs/`) on push to `main`/`master`.
- **release-please.yml** — maintains the release PR (version bump +
  `CHANGELOG.md`) on push to `master`.
- **release.yml** — on a `vX.Y.Z` tag: builds + pushes the API image, publishes
  the frontend bundle + OpenAPI spec on a GitHub Release, optionally deploys the
  VM.
- **deploy-backend.yml** — manual (`workflow_dispatch`): pin the VM to any
  existing GHCR image tag — redeploy or rollback.
- **frontend-unit-tests.yml** — type-checks the frontend and runs its Vitest
  unit suite. No backend or database needed, so it's the fastest signal on a
  frontend change.
- **frontend-integration.yml** — runs the Playwright end-to-end suite against a
  MySQL service container.
- **periodic-tests.yml** — runs the Cargo test suite on a schedule and on every
  push/PR.

## Contributing

Issues and pull requests are welcome. Please open an issue to discuss any
significant change before submitting a PR.

## License

This project is licensed under the [Apache License 2.0](LICENSE).
