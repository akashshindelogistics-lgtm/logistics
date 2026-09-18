# Multi-stop trips

Until now a dispatch meant one vehicle → one customer. A **trip** lets one
vehicle carry several customers' orders in a sequence.

## Shape

A trip is a thin grouping over ordinary dispatches:

- `Trips` table: `id, org_id, vehicle_registration_number, created_at`.
- `Dispatches` gained `trip_id` + `stop_sequence` (both `NULL` for a
  standalone one-customer dispatch). Added to `CREATE TABLE` and back-filled
  onto an older local database by `ensure_trip_columns`.
- Each stop is a **full `DispatchOrder`** — its own `PENDING → …` lifecycle,
  its own invoice, its own proof of delivery. You advance a trip by advancing
  its stops through `PUT /api/dispatches/{id}/status` like any other dispatch.
- `Trip::status` is *derived* from the stops:
  `PLANNED` (all `PENDING`) → `IN_PROGRESS` → `COMPLETED` (all terminal).

## Creating one — `Organization::dispatch_trip_to_customers`

The dispatch flow was refactored into three shared helpers so a trip and a
single dispatch plan and draw stock the same way:

- `stock_snapshot` / `plan_stock_draw` — validate a customer's lines against a
  **decrementing** snapshot of every godown's holdings, so two stops on one
  trip can't over-commit the same item.
- `select_free_vehicle` — an org vehicle with an active driver, spare
  capacity, and no non-terminal dispatch, nearest to a target customer.
- `draw_down_plans` — apply the planned draws to `Stock`.

A trip:

1. needs **≥ 2 stops** and no customer twice (use `/dispatch` for one customer);
2. every stop's customer must belong to the org and have a location;
3. plans every stop against the shared snapshot, summing the volume;
4. fits the **whole trip** onto a single free vehicle (nearest to stop 1) —
   `400` if nothing is free and large enough;
5. is **all-or-nothing** — nothing is drawn and no `Trips`/`Dispatches` rows
   are written unless every stop succeeds;
6. writes the `Trips` row and one `Dispatches` row per stop
   (`stop_sequence` 1…N, all on the chosen vehicle), each `PENDING`, and fires
   the create-notification for each stop's customer + the driver.

## Routes (Admin | Dispatcher)

| Method & path | |
| --- | --- |
| `POST /api/orgs/{id}/trips` | `{ stops: [{ customer_id, line_items: [{ stock_description, requested_quantity }] }], optimize_route? }` → the created `Trip` with its stops |
| `GET /api/orgs/{id}/trips` | the org's trips, newest first, each with its stops + derived status |
| `GET /api/trips/{id}` | one trip and its stops (owned) |

## Route optimization

`optimize_route: true` on create reorders stops 2..N by geography before
planning — `stops[0]` is always kept as the route's fixed starting point (and
the point vehicle selection still anchors on, so which vehicle gets picked
never changes). Defaults to `false`: visit `stops` in the exact order given,
the original behaviour.

The ordering itself is a greedy **nearest-neighbour** heuristic
(`src/logistics/dispatch/route.rs::nearest_neighbor_order`) over
`haversine_distance_km` great-circle distance: starting from stop 1, repeatedly
hop to whichever remaining stop is closest to wherever the walk currently is.
It's pure math with no I/O, so it's unit-tested standalone with no database.
This is a heuristic, not an exact solution — true "shortest tour visiting
every point" (TSP) is NP-hard — but nearest-neighbour is a fast, standard
approximation that works well for the handful of stops a real trip has.

Reordering happens before stock is planned (not after), because *which*
stop's request succeeds first matters when two stops on a shared, decrementing
stock snapshot want more of an item than is on hand — so the final visiting
order has to be settled before `plan_stock_draw` runs, not applied as a
cosmetic re-sort of the result afterward.

## Frontend

A **Trips** page (`/trips`, sidebar link): a "Plan a Trip" form with
repeatable stop rows (customer + one stock line + quantity), an "Optimize
stop order" checkbox, and a card per trip showing the vehicle, the derived
status, and the ordered stops with each stop's live dispatch status. The
**Dispatches** page tags a trip stop's row with `Trip · stop N`.

## Not done here (follow-ups)

- The UI's trip form takes one stock line per stop; the API takes many.
- The nearest-neighbour heuristic doesn't attempt 2-opt or any other
  tour-improvement pass — good enough for a handful of stops, but it can be
  measurably worse than optimal on a larger or more adversarial one.
- A trip has no status/actions of its own; you drive it stop by stop from the
  Dispatches page.

## Tests

- Rust pure logic (`dispatch/route.rs`): `haversine_distance_km` against a
  known approximate distance; `nearest_neighbor_order` is the identity on an
  already-sorted input, reorders an out-of-order one, is a permutation of
  every index, handles empty/single-point input, and never produces a longer
  route than the input order on a scattered case.
- Rust model (`orgs.rs`): a trip puts every stop on one vehicle, sequences
  them, and draws each stop's stock down; `< 2` stops / a repeated customer /
  no vehicle big enough for the combined load are rejected; a short later
  stop rolls the whole trip back; `optimize_route: true` reorders stops 2..N
  by proximity while keeping stop 1 fixed, `false` (the default) keeps the
  caller's exact order.
- Rust routes (`routes.rs`): create a two-stop trip and read it back via
  `GET /trips/{id}`, `/orgs/{id}/trips`, and `/dispatches` (stops linked by
  `trip_id`); one stop / unknown customer → `400`; warehouse-staff role →
  `403`; `optimize_route: true` reorders stops by proximity end-to-end
  through the API.
- Frontend unit: `trips` api client (`optimize_route` defaults false, passes
  through when requested); the Trips page lists trips and their stops,
  rejects `< 2` complete stops, posts a well-formed payload, and passes
  `optimizeRoute: true` when the checkbox is checked.
- Playwright (`trips.spec.ts`): plan a two-stop trip through the UI and see
  both stops share one vehicle and get tagged on the Dispatches page, plus a
  scenario checking the "Optimize stop order" checkbox and confirming the
  farther-away stop is visited after the nearer one. Full-flow demo has a
  "plan a multi-stop trip to two customers" step;
  `npm run test:e2e:demo:trips` is the standalone headed walkthrough.
