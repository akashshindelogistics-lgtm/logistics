# Changelog

All notable changes to this project are documented here. From v0.1.1 on, this
file is maintained automatically by [release-please](https://github.com/googleapis/release-please)
from the conventional-commit history.

## 0.2.0

Cut manually — the automated `release-please` release-PR flow was blocked by
a repo permissions setting, so this entry is hand-written rather than
generated from conventional commits (normal going forward once that's fixed).

### Features

- **Multi-stop trips**: one vehicle can now carry several customers' orders
  in a single trip, instead of one dispatch always meaning one vehicle to one
  customer. Each stop is a full dispatch with its own lifecycle, invoice and
  proof of delivery.
- **Multi-line-item dispatches**: a single shipment can carry several stock
  lines instead of one.
- **Stock transfer** between two godowns of the same org, with an audit
  trail of the move.
- **Freight billing**: an invoice per dispatch, amend-while-unpaid, mark
  paid, and a customer's outstanding/overdue billing summary.
- **Returns**: moving a dispatch to `RETURNED` credits its stock back into a
  godown.
- **Role-scoped team members** within an organization (Admin / Dispatcher /
  Warehouse Staff), on top of the existing org-owner login.
- **GPS tracker push endpoint** — a device can report a vehicle's live
  location with no login, keyed by a per-vehicle rotating tracker key.
- **Vehicle compliance paperwork** (insurance, RC, permit, PUC, fitness
  certificate) with expiry tracking and renewal reminders.
- **Customer contact details and notifications**: an email/SMS-shaped
  notification is recorded when a dispatch is created and when it's
  delivered.
- **Operational reporting**: fleet utilization, delivery performance,
  per-godown inventory, and a 14-day dispatch-volume series.
- Customers are now scoped to their organization (previously a single flat,
  shared table).
- Dashboard gained detail/edit pages for vehicles, drivers and godowns, and a
  UI form to add stock to a godown.
- CI now runs on a push to any branch, not just after a PR is opened; the
  stale `main` branch was removed and `master` is the GitHub default branch.

### Fixes

- The Docker/release image build failed because Swagger UI wasn't vendored;
  it's now bundled into the build.

## 0.1.0 (first release)

First tagged release of the Logistics System — a Rust/Actix REST API backed by
MySQL with a React + TypeScript dashboard.

### Features

- **Organizations** with an editable location, and org-level login (JWT +
  bcrypt).
- **Godowns (warehouses)** per organization, each holding **stock** identified
  by description. Optional `max_capacity` per godown (rejects over-fill with
  `409`) and an optional per-item `reorder_threshold` that surfaces a
  `below_threshold` flag.
- **Vehicles** per organization with live location and a rated `capacity`;
  units of measure `MetricTon` / `Kg` / `Litre` / `Box` / `Pallet` / `Piece`.
- **Drivers** per organization (name, licence number, phone, active flag),
  assignable to a vehicle from the dashboard.
- **Customers** with a delivery location.
- **Dispatches** modelled as a lifecycle
  (`PENDING → CONFIRMED → LOADED → IN_TRANSIT → DELIVERED`/`RETURNED`/`CANCELLED`)
  with a timestamped status history. A dispatch only selects a vehicle that has
  an **active assigned driver**, spare **capacity** for the shipment, and no
  trip already in progress. Reaching `DELIVERED` requires **proof of delivery**
  (receiver name + signature/photo).
- **AI dispatch summaries** via the Anthropic (Claude) API.
- **Interactive API docs** — Swagger UI generated from the API with `utoipa`,
  deployed to GitHub Pages alongside the dashboard.

### Deployment

- Frontend + Swagger UI on **GitHub Pages**; Rust API + MySQL on an **Oracle
  Cloud "Always Free"** VM via `docker compose` (Caddy auto-HTTPS, DuckDNS).
- Backend image built natively for `linux/arm64` and published to GHCR.
- `JWT_SECRET` is now required for release builds (the server refuses to start
  without a 32+ character value).
