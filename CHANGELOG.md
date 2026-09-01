# Changelog

All notable changes to this project are documented here. From v0.1.1 on, this
file is maintained automatically by [release-please](https://github.com/googleapis/release-please)
from the conventional-commit history.

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
