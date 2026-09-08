# Operational reporting

`GET /api/orgs/{id}/reports` (bearer auth, must be your own org — `403`
otherwise) returns an `OpsReport` for one organisation. Everything is
**derived** — there are no report tables. `OpsReport::for_org`
(`src/logistics/reports/mod.rs`) loads the org's dispatches, vehicles and
godowns through their existing list methods and folds them into the figures
below, the same way `Invoice::customer_summary` works off the full invoice
list.

## Figures

| Field | Definition |
| --- | --- |
| `vehicle_utilization.total_vehicles` | Vehicles the org owns. |
| `vehicle_utilization.vehicles_on_active_trip` | Distinct owned vehicles with a dispatch in a **non-terminal** status (`PENDING`/`CONFIRMED`/`LOADED`/`IN_TRANSIT`). |
| `vehicle_utilization.utilization_percent` | `active / total × 100`, one decimal. `0.0` when the org has no vehicles. |
| `delivery_performance.delivered_count` / `returned_count` | Dispatches currently in `DELIVERED` / `RETURNED`. |
| `delivery_performance.avg_hours_to_deliver` | Mean hours from a dispatch's creation to its `DELIVERED` status-history event, over all delivered dispatches. `null` when none. |
| `delivery_performance.on_time_rate_percent` | Share of delivered dispatches that reached `DELIVERED` within **72 hours** (`ON_TIME_TARGET_HOURS`). `null` when none. |
| `units_dispatched_recently` | Σ(line-item quantity) over dispatches **created in the last 30 days**, any status. |
| `godown_inventory[]` | Per godown: `units_on_hand` (Σ quantity), `distinct_items`, and `capacity_used_percent` = Σ(volume × quantity) / `max_capacity` × 100 (`null` when the godown has no cap). |
| `dispatch_volume[]` | One `{ date, count }` point per day for the last **14 days**, oldest first, UTC. Days with no dispatches are present with `count: 0`. |

## Known limitations

- **On-time rate** uses a single fleet-wide 72-hour target because there is no
  per-order promised/SLA date yet. When such a field lands, swap
  `ON_TIME_TARGET_HOURS` for a per-dispatch comparison.
- **Turnover is org-wide, not per-godown.** A dispatch draws stock from the
  org's godowns but the draw is not recorded per godown, so
  `units_dispatched_recently` cannot be attributed to one. `godown_inventory`
  therefore reports what each godown *holds*, not its throughput.
- The report loads the org's full dispatch list in memory. Fine at this app's
  scale (same approach as the billing summary); revisit with SQL aggregation
  if dispatch counts ever get large.

## Frontend

A **Reports** page (`/reports`, sidebar link) with four headline stat tiles
(fleet utilization, delivered, average time to deliver, on-time rate), a
CSS-bar chart of the 14-day dispatch volume, and a godown-inventory table.
Dependency-free — no chart library. `frontend/src/api/reports.ts` exposes
`getOpsReport(orgId)`.

## Tests

- Rust model (`reports/tests.rs`): empty-org zeros; utilization counts only
  non-terminal trips; delivery averages + on-time split across a fast and a
  back-dated slow delivery; volume buckets by calendar day incl. the 14-day
  cutoff; godown capacity maths; the 30-day cutoff for dispatched units.
- Rust route (`routes.rs`): the report reflects a live dispatch and its
  vehicle; `403` for another org.
- Frontend unit: `reports` api client; the Reports page renders tiles, the
  godown table, the 14 volume bars, and an empty state.
- Playwright (`reports.spec.ts`): a fresh org shows a zeroed report with the
  full 14-day axis; a dispatch then moves utilization to `100%`, adds today's
  volume bar, and shows the drawn-down godown inventory.
