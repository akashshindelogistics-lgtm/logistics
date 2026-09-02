# Editing entity details in the dashboard

Tracked by the `todo.org` item *"At the moment there is no option to edit any
of the details on the UI Dashboard. Like editing vehicle, drivers, godown
details. Clicking on each one should open another details page where
information can be updated and saved."*

## Detail pages

Each of the three entities gets its own route. Clicking the entity's name in a
list navigates there; the page loads the entity, shows an editable form
pre-filled with its current values, and a **Save** button that PUTs the
change and shows a confirmation.

| Route | Component | Editable fields | Reached from |
|---|---|---|---|
| `/vehicles/:reg` | `VehicleDetail` | capacity, unit | registration link in the Fleet Vehicles table |
| `/drivers/:id` | `DriverDetail` | name, licence number, phone, active flag | driver-name link in the org detail Drivers table |
| `/godowns/:id` | `GodownDetail` | name, address, max capacity | godown-name link in the org detail Godowns section |

`VehicleDetail` and `DriverDetail` load their entity by filtering the
org-scoped list endpoint (`GET /api/vehicles`, `GET /api/drivers`) — a
vehicle/driver that isn't in the caller's list renders a "not found" state.
`GodownDetail` uses the existing `GET /api/godowns/{gid}`.

## Backend

Drivers and godowns already had update endpoints (`PUT /api/drivers/{id}`,
`PUT /api/godowns/{gid}`); this change adds the missing one for vehicles:

- **`PUT /api/vehicles/{reg}`** `{ capacity, unit }` — `edit_vehicle` in
  `routes.rs`. It first calls `check_owned_vehicle`, which looks up the
  vehicle's `org_id` (`Vehicle::org_of`) and returns `403` if it belongs to
  another org or `404` if the registration number is unknown, then applies
  `Vehicle::update_vehicle`.

The pre-existing `PUT /api/vehicles/{reg}/location` and
`DELETE /api/vehicles/{reg}` handlers still operate by registration number
without an ownership check — a separate gap, left as-is here.

The `updateGodown` API client now always sends `max_capacity` (defaulting to
`null`, which the backend treats as "no cap"), so the godown detail form can
both set and clear the cap.
