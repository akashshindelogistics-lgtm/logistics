# Logistics System

[![Frontend Integration Tests (Playwright)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/frontend-integration.yml/badge.svg)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/frontend-integration.yml)

[![Periodic Cargo Tests](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/periodic-tests.yml/badge.svg)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/periodic-tests.yml)

[![Swagger UI — Validate & Deploy to GitHub Pages](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/swagger-pages.yml/badge.svg)](https://github.com/akashshindelogistics-lgtm/logistics/actions/workflows/swagger-pages.yml)

A logistics management platform for tracking organizations, their vehicle
fleets, stock, customers, and delivery dispatches. It ships as a Rust REST
API backed by MySQL, with a React + TypeScript dashboard for day-to-day
operations, live location maps, and AI-generated dispatch summaries.

📖 **Live API docs (Swagger UI):** <https://akashshindelogistics-lgtm.github.io/logistics/>

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
  [GitHub Pages](https://akashshindelogistics-lgtm.github.io/logistics/) on
  every push to `main`/`master`.
- **Web dashboard** — a React frontend for organizations, vehicles, stock,
  customers, and dispatches, including a live Leaflet map of locations.

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

# Frontend end-to-end tests (Playwright)
cd frontend
npm run test:e2e
```

## API overview

Key REST endpoints exposed by the server (see the
[published Swagger UI](https://akashshindelogistics-lgtm.github.io/logistics/)
for the full spec, request/response schemas, and try-it-out support):

| Method | Path | Description |
|---|---|---|
| POST | `/auth/login` | Log in to an organization |
| GET | `/auth/me` | Get the authenticated organization |
| GET | `/auth/orgs` | List organizations available for login |
| GET/POST | `/orgs` | List / create organizations |
| GET/PUT/DELETE | `/orgs/{id}` | Get, update, or delete an organization |
| PUT | `/orgs/{id}/location` | Update an organization's location |
| GET/POST | `/orgs/{id}/godowns` | List / create an organization's godowns (warehouses) |
| GET/PUT/DELETE | `/godowns/{gid}` | Get, rename/re-address, or delete a godown |
| PUT | `/godowns/{gid}/location` | Update a godown's location |
| POST/PUT/DELETE | `/godowns/{gid}/stock`, `/godowns/{gid}/stock/{desc}` | Add, update, or remove a godown's stock |
| POST | `/orgs/{id}/vehicles` | Add a vehicle to an organization |
| GET/DELETE | `/vehicles`, `/vehicles/{reg}` | List vehicles / remove one |
| PUT | `/vehicles/{reg}/location` | Update a vehicle's location |
| GET/POST | `/customers` | List / create customers |
| PUT | `/customers/{id}/location` | Update a customer's location |
| POST | `/orgs/{id}/dispatch` | Dispatch stock from an org to a customer |
| GET | `/dispatches` | List dispatch orders |
| PUT | `/dispatches/{id}/status` | Advance a dispatch's lifecycle status |
| GET | `/dispatches/{id}/summary` | AI-generated summary of a dispatch |
| GET | `/health` | Health check |

The published Swagger UI (see CI/CD below) has "Try it out" enabled and its
OpenAPI spec declares `http://127.0.0.1:8080` as the server, so if you run
the backend locally (`cargo run`), you can exercise the live API straight
from the hosted docs — the server enables permissive CORS for this. There
is no publicly hosted backend yet, so this only works against a local
instance.

## CI/CD

GitHub Actions workflows in `.github/workflows/`:

- **swagger-pages.yml** — validates the OpenAPI spec and deploys the
  Swagger UI to
  [GitHub Pages](https://akashshindelogistics-lgtm.github.io/logistics/) on
  push to `main`/`master`.
- **frontend-integration.yml** — runs the Playwright end-to-end suite
  against a MySQL service container.
- **periodic-tests.yml** — runs the Cargo test suite on a schedule and on
  every push/PR.

## Contributing

Issues and pull requests are welcome. Please open an issue to discuss any
significant change before submitting a PR.

## License

This project is licensed under the [Apache License 2.0](LICENSE).
