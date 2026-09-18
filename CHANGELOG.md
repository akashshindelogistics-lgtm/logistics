# Changelog

All notable changes to this project are documented here. From v0.1.1 on, this
file is maintained automatically by [release-please](https://github.com/googleapis/release-please)
from the conventional-commit history.

## [0.8.0](https://github.com/akashshindelogistics-lgtm/logistics/compare/v0.7.0...v0.8.0) (2026-09-18)


### Features

* **billing:** default a new invoice's due date to net-30 ([b7d285d](https://github.com/akashshindelogistics-lgtm/logistics/commit/b7d285d050f283393c1f1c90cf72658191c51550))
* **dispatches:** real file storage for proof-of-delivery photos ([9feeeae](https://github.com/akashshindelogistics-lgtm/logistics/commit/9feeeae08f80b0cd4b4ecff9321c09873c9cb0f3))
* **notifications:** delay/ETA-drift alerts for in-transit dispatches ([36a70a1](https://github.com/akashshindelogistics-lgtm/logistics/commit/36a70a114ea41eff5d74ffb58b8335926de4be6a))
* **trips:** live vehicle location tracking on multi-stop trips ([e521d65](https://github.com/akashshindelogistics-lgtm/logistics/commit/e521d659e1fc6cd64ea880d10a543674aeb927fb))
* **trips:** opt-in route optimization for multi-stop trip creation ([de470cb](https://github.com/akashshindelogistics-lgtm/logistics/commit/de470cbd327a2ad6ef8bbe6c89d217854bd6d3a1))
* **trips:** show a trip's vehicle location alongside its planned stops ([1accbd9](https://github.com/akashshindelogistics-lgtm/logistics/commit/1accbd95aebd8d1b9a6f31d7937974ef2485b11c))
* **ui:** add a light/dark theme toggle ([3030cd2](https://github.com/akashshindelogistics-lgtm/logistics/commit/3030cd2f6b3032110f2fc970d0900246096a446c))
* **vehicles:** preventive maintenance scheduling by date and/or mileage ([9ee2dd1](https://github.com/akashshindelogistics-lgtm/logistics/commit/9ee2dd13b2bc72c34ef0e28b216a244c3e34b9e9))


### Bug Fixes

* **trips:** decouple location-tracking commit from the uncommitted route-optimization feature ([7f4c167](https://github.com/akashshindelogistics-lgtm/logistics/commit/7f4c1673749e5fe7a96539437905b47e3585d9d7))

## [0.7.0](https://github.com/akashshindelogistics-lgtm/logistics/compare/v0.6.0...v0.7.0) (2026-09-13)


### Features

* **ai:** personalize notification copy with Claude, falling back to the fixed template ([575d9b8](https://github.com/akashshindelogistics-lgtm/logistics/commit/575d9b8a5961d3d955c7f3c60ddf9b39b16b1e8c))
* **ai:** personalize notification copy with Claude, falling back to the fixed template ([d60b8d2](https://github.com/akashshindelogistics-lgtm/logistics/commit/d60b8d2fe90e0f9a48fe7b99a271b0e89f61e2d2))
* **notifications:** actually deliver notifications via Twilio and Resend ([61ff27a](https://github.com/akashshindelogistics-lgtm/logistics/commit/61ff27a189794ff7bdeab71ba57d15456d4acb1a))

## [0.6.0](https://github.com/akashshindelogistics-lgtm/logistics/compare/v0.5.0...v0.6.0) (2026-09-13)


### Features

* **ai:** add an AI-narrated "explain this report" summary ([f14ccd1](https://github.com/akashshindelogistics-lgtm/logistics/commit/f14ccd175f5e9267dcc9afa1dc63861755c16f27))
* **ai:** add an AI-narrated "explain this report" summary ([fa24244](https://github.com/akashshindelogistics-lgtm/logistics/commit/fa242442d0ac678b8dbb0196840665bbb24743d9))
* **ai:** add an on-demand daily ops digest to the assistant widget ([a1ac138](https://github.com/akashshindelogistics-lgtm/logistics/commit/a1ac138b75cd6a2e6d888d4433a359be3bedaba8))
* **ai:** add an on-demand daily ops digest to the assistant widget ([476c7e0](https://github.com/akashshindelogistics-lgtm/logistics/commit/476c7e0d7c17aedfe728dac5b00a29cb9d693c12))
* **ai:** add smart reorder suggestions based on dispatch velocity ([0c666f3](https://github.com/akashshindelogistics-lgtm/logistics/commit/0c666f336f248d08f6bab44d79e25e0e0507be65))
* **ai:** add smart reorder suggestions based on dispatch velocity ([eb09481](https://github.com/akashshindelogistics-lgtm/logistics/commit/eb094812f7a11b398089acc68dd7450e73310ac6))
* **ai:** ground the assistant in a live OpsReport snapshot (Phase 4) ([16d77b6](https://github.com/akashshindelogistics-lgtm/logistics/commit/16d77b623802d5c72054bb63abfbac47bed63a26))
* **ai:** ground the assistant in a live OpsReport snapshot (Phase 4) ([e5aac68](https://github.com/akashshindelogistics-lgtm/logistics/commit/e5aac68150222d30ce1de986cab7eebc6a20e9af))


### Bug Fixes

* **assistant:** keep the digest/reorder suggestions reachable after the first question ([94d76d9](https://github.com/akashshindelogistics-lgtm/logistics/commit/94d76d9bca0f2ce0e2a69de907a2324e7fa7faef))
* **assistant:** keep the digest/reorder suggestions reachable after the first question ([ba68949](https://github.com/akashshindelogistics-lgtm/logistics/commit/ba689499cc1b4a27bf7301c7a87b6d9a4670a61e))


### Documentation

* **todo:** mark the "ask your data" assistant parent item done ([6d0e90f](https://github.com/akashshindelogistics-lgtm/logistics/commit/6d0e90f5c30588644d68342a5f9465210d7cb3f0))
* **todo:** mark the "ask your data" assistant parent item done ([380652b](https://github.com/akashshindelogistics-lgtm/logistics/commit/380652b2e586ea69a29d492838844ef1ad5dbaec))

## [0.5.0](https://github.com/akashshindelogistics-lgtm/logistics/compare/v0.4.0...v0.5.0) (2026-09-12)


### Features

* **ai:** index dispatch narratives and stock snapshots for the assistant ([e3ebe22](https://github.com/akashshindelogistics-lgtm/logistics/commit/e3ebe22db4ade44bd322499a1f2467e0af022be5))
* **ai:** index dispatch narratives and stock snapshots for the assistant ([727f915](https://github.com/akashshindelogistics-lgtm/logistics/commit/727f915efe709c7ca0617cf184371deb68c44d93))
* **ai:** index the org directory and add a full reindex backfill ([ce3191e](https://github.com/akashshindelogistics-lgtm/logistics/commit/ce3191ebff788521f6f10365c1856f71b2c86337))
* **ai:** index the org directory and add a full reindex backfill ([fa1de13](https://github.com/akashshindelogistics-lgtm/logistics/commit/fa1de137d8691560d6330bf4dc899a5ce1ca001b))

## [0.4.0](https://github.com/akashshindelogistics-lgtm/logistics/compare/v0.3.0...v0.4.0) (2026-09-12)


### Features

* **ai:** org-scoped "ask your data" assistant (Phase 1) ([93d0544](https://github.com/akashshindelogistics-lgtm/logistics/commit/93d0544597f9f8b0faeecd35c7adcd9421683631))
* **ai:** org-scoped "ask your data" assistant (Phase 1) ([261cbb9](https://github.com/akashshindelogistics-lgtm/logistics/commit/261cbb9f2a768aea29bb4ce555cb92633e2f8c90))


### Documentation

* **todo:** add AI/RAG roadmap ([7d41cd9](https://github.com/akashshindelogistics-lgtm/logistics/commit/7d41cd9ac1a543fc5a09a64a703b840e7fb87a48))
* **todo:** add AI/RAG roadmap ([293bcc2](https://github.com/akashshindelogistics-lgtm/logistics/commit/293bcc26f9ee15106c54df905e3a48eba404fd70))

## [0.3.0](https://github.com/akashshindelogistics-lgtm/logistics/compare/v0.2.1...v0.3.0) (2026-09-11)


### Features

* **ai:** send anthropic-workspace-id when the API key is identity-li… ([c90a523](https://github.com/akashshindelogistics-lgtm/logistics/commit/c90a52376b41506b8d8a0cc945f9c898357700c3))
* **ai:** send anthropic-workspace-id when the API key is identity-linked ([2d14f73](https://github.com/akashshindelogistics-lgtm/logistics/commit/2d14f73c2f5798d90d7fae265ff5d223137d0b95))
* **auth:** role-scoped team members within an organization ([453cddb](https://github.com/akashshindelogistics-lgtm/logistics/commit/453cddb8ae6f45e942e2717b3cfb159058789bed))
* **billing:** freight invoices with per-customer payment status ([9a02dc9](https://github.com/akashshindelogistics-lgtm/logistics/commit/9a02dc97de00f59e3a8bcb5ca95e3fbc1e09f8b5))
* **billing:** freight invoices with per-customer payment status ([67bd75b](https://github.com/akashshindelogistics-lgtm/logistics/commit/67bd75b10f95bc9e7e1e65744fcc62408604f472))
* **customer:** optional location on the create-customer form ([9d3153b](https://github.com/akashshindelogistics-lgtm/logistics/commit/9d3153bab950dabeba165a56521dc124a266c1fd))
* **customer:** optional location on the create-customer form ([ca722c8](https://github.com/akashshindelogistics-lgtm/logistics/commit/ca722c895be3674972478ac40fdc520451320240))
* **customer:** scope customers to a single org, never shared ([0ef3899](https://github.com/akashshindelogistics-lgtm/logistics/commit/0ef38990ba501c969bfa9299fb26467e18bd07ea))
* **customer:** scope customers to a single org, never shared ([531c8e2](https://github.com/akashshindelogistics-lgtm/logistics/commit/531c8e20e5355538e17606da62b8d3fcaababf5b))
* database-per-test isolation, restore parallelism (Phase 3) ([2e4fa67](https://github.com/akashshindelogistics-lgtm/logistics/commit/2e4fa67d3a0646e5f797bc572f41c772fb668ed1))
* database-per-test isolation, restore parallelism (Phase 3) ([9c94c36](https://github.com/akashshindelogistics-lgtm/logistics/commit/9c94c360a1fe3419a70c5b4557dfbf64ec8bf5a3))
* DbConfig::from_env() - honour DATABASE_URL / MYSQL_* env vars ([0716014](https://github.com/akashshindelogistics-lgtm/logistics/commit/0716014887bb80f6e197c82572e9408632b2cea3))
* DbConfig::from_env() - honour DATABASE_URL / MYSQL_* env vars ([673e60c](https://github.com/akashshindelogistics-lgtm/logistics/commit/673e60c97c08caf6b850a2a224110cf02ce1a171))
* **dispatch:** carry multiple stock line items in one dispatch ([43313bd](https://github.com/akashshindelogistics-lgtm/logistics/commit/43313bd538cf37ca0df8aa887ea9e7d35de9d412))
* **dispatch:** carry multiple stock line items in one dispatch ([85424f4](https://github.com/akashshindelogistics-lgtm/logistics/commit/85424f4b2039c04fa463dcb713e5c049e19f8c23))
* **dispatch:** multi-stop trips — one vehicle, several customer stops ([9e30efa](https://github.com/akashshindelogistics-lgtm/logistics/commit/9e30efa44c393e686081c23e408a491e531947d2))
* **dispatch:** multi-stop trips — one vehicle, several customer stops ([322ef67](https://github.com/akashshindelogistics-lgtm/logistics/commit/322ef674cc3fcb144f7d57486fd1b204d22cc066))
* **dispatch:** returns credit stock back into a godown ([6306e51](https://github.com/akashshindelogistics-lgtm/logistics/commit/6306e51f442387d96c48278863744cd8dbf296ab))
* **dispatch:** vehicle capacity check and no double-booking ([f79676b](https://github.com/akashshindelogistics-lgtm/logistics/commit/f79676b2a2f4626d874b3f5796f226f3508c213c))
* **driver:** Driver entity and active-driver-required-for-dispatch g… ([342755a](https://github.com/akashshindelogistics-lgtm/logistics/commit/342755a5a0bd944508a30129ffe3c6ae214183d8))
* **driver:** Driver entity and active-driver-required-for-dispatch guard ([f5bed5c](https://github.com/akashshindelogistics-lgtm/logistics/commit/f5bed5cb94543e65f7fb95d139fd65859746d9a7))
* **e2e:** add a headed, narrated full-workflow demo test ([856a222](https://github.com/akashshindelogistics-lgtm/logistics/commit/856a2225a32e0d4df44047a57a9ee87ff8ea354a))
* **e2e:** extend the demo flow to cover freight billing and returns ([28a49ab](https://github.com/akashshindelogistics-lgtm/logistics/commit/28a49aba9e289e06e1c9864466e39e2088617bd4))
* **e2e:** extend the demo flow to cover freight billing and returns ([1de35f4](https://github.com/akashshindelogistics-lgtm/logistics/commit/1de35f4bbce8abcce665c5072711f5075a9030cb))
* **frontend:** Drivers UI + fix dispatch e2e for the new preconditions ([98e2a44](https://github.com/akashshindelogistics-lgtm/logistics/commit/98e2a44cbd553fce8de471d37d1264376e96c8a8))
* **frontend:** sync dispatch UI with the lifecycle + proof-of-delivery API, add Vitest unit tests ([9337b3f](https://github.com/akashshindelogistics-lgtm/logistics/commit/9337b3f0b0b60fdd78ee0d0cb9325665c70f65de))
* **godown:** max_capacity cap and per-stock-item reorder threshold ([d5f979d](https://github.com/akashshindelogistics-lgtm/logistics/commit/d5f979d28e67ebc6b79b6f10f9ba5df241d14c58))
* **godown:** stock transfer between two godowns with an audit trail ([311024e](https://github.com/akashshindelogistics-lgtm/logistics/commit/311024e7ef683aabf4d80489ecdb9cd962afacbc))
* **godown:** stock transfer between two godowns with an audit trail ([3e69df7](https://github.com/akashshindelogistics-lgtm/logistics/commit/3e69df7b0c28b26cdfb58dc285ef251171b406a8))
* model dispatch as a lifecycle with a timestamped status history ([52eb604](https://github.com/akashshindelogistics-lgtm/logistics/commit/52eb6040ac91795c6f4370c434c2f4c567687b0a))
* multi-godown warehouses for organizations (backend + UI) ([c3d5413](https://github.com/akashshindelogistics-lgtm/logistics/commit/c3d5413b104a822a4756446f29eafdd2e2885db3))
* multi-godown warehouses for organizations (backend + UI) ([9e2bbd4](https://github.com/akashshindelogistics-lgtm/logistics/commit/9e2bbd40b028fc8d039c4830052d42d1b84a6017))
* **notifications:** record customer & driver notifications for a dispatch ([08e23d7](https://github.com/akashshindelogistics-lgtm/logistics/commit/08e23d7983cf4fe8281f954d693577c130c9653f))
* **reports:** basic operational reporting per organization ([2185ddb](https://github.com/akashshindelogistics-lgtm/logistics/commit/2185ddbae0d0606b0b911cb19f79ef38bdbc8681))
* **reports:** basic operational reporting per organization ([1dadf04](https://github.com/akashshindelogistics-lgtm/logistics/commit/1dadf04449714769bb44219e9d15ce4396f9dc1a))
* require proof of delivery before a dispatch can be marked DELIVERED ([2d2b5af](https://github.com/akashshindelogistics-lgtm/logistics/commit/2d2b5af643583459175688ac094523e15102567d))
* **ui:** detail pages to edit vehicle, driver and godown details ([09bcd8b](https://github.com/akashshindelogistics-lgtm/logistics/commit/09bcd8b01a1e5e5dacda0aeb5b16f82b1fb805d0))
* **ui:** detail pages to edit vehicle, driver and godown details ([a59118d](https://github.com/akashshindelogistics-lgtm/logistics/commit/a59118d8d92757cb546e934e0ab953f20cebdb12))
* **vehicle:** add Kg, Litre, Box, Pallet, Piece units of measure ([ed4ce03](https://github.com/akashshindelogistics-lgtm/logistics/commit/ed4ce03a35345057804a43e4f912e1c945620ec7))
* **vehicle:** automatic GPS location push via a per-vehicle tracker key ([bd4d788](https://github.com/akashshindelogistics-lgtm/logistics/commit/bd4d788ca7a9665a7b3529270033633d535d326f))
* **vehicle:** automatic GPS location push via a per-vehicle tracker key ([8f52da0](https://github.com/akashshindelogistics-lgtm/logistics/commit/8f52da0cf4627e2cf3d46c90ee9dcea6f1eafe26))
* **vehicle:** compliance paperwork tracking with expiry reminders ([8486e74](https://github.com/akashshindelogistics-lgtm/logistics/commit/8486e74dfa75b0b907ba45d1c090a41fb4522db5))
* **vehicle:** compliance paperwork tracking with expiry reminders ([26770be](https://github.com/akashshindelogistics-lgtm/logistics/commit/26770be797c9121f6ca6210436d40bd50d3bf5eb))


### Bug Fixes

* **ai:** update dispatch-summary test fixture for the new trip fields ([ccf7e80](https://github.com/akashshindelogistics-lgtm/logistics/commit/ccf7e8082a1cc5e3f7f500108aad88cf67698a54))
* **ai:** update dispatch-summary test fixture for the new trip fields ([49ef5e9](https://github.com/akashshindelogistics-lgtm/logistics/commit/49ef5e95eaf44b010406d61eb1cfaacf4454bed4))
* **build:** vendor Swagger UI so the Docker/release image build works ([4020689](https://github.com/akashshindelogistics-lgtm/logistics/commit/40206893164be28f853ccd15fe6fadd78fe0cb93))
* **build:** vendor Swagger UI so the Docker/release image build works ([0e14d27](https://github.com/akashshindelogistics-lgtm/logistics/commit/0e14d27096a98da87156e3969df203336c67a518))
* **ci:** build the API image natively on arm64 instead of via QEMU ([89fe438](https://github.com/akashshindelogistics-lgtm/logistics/commit/89fe438786157e9b829e00cb61a9c2e012e7254f))
* **ci:** build the API image natively on arm64 instead of via QEMU ([53815cd](https://github.com/akashshindelogistics-lgtm/logistics/commit/53815cd2e57b160ffa7db7040908149a06f53623))
* **compliance:** collapse the add-document form behind a toggle ([d98d7b2](https://github.com/akashshindelogistics-lgtm/logistics/commit/d98d7b215bed8854f575c8122540b9fda4111f34))
* **e2e:** wait for detail-page render before asserting driver/godown fields ([29c66a9](https://github.com/akashshindelogistics-lgtm/logistics/commit/29c66a957e789d9417510bb2d3160b768f3e3214))


### Documentation

* add ground-ops research backlog for dispatch lifecycle & fleet gaps ([e55ac54](https://github.com/akashshindelogistics-lgtm/logistics/commit/e55ac54cbf789618229eb96c9c744cf99daa1a30))
* add hosted Swagger UI GitHub Pages URL to README ([78718cf](https://github.com/akashshindelogistics-lgtm/logistics/commit/78718cfc16db6600553784f2c8409c6e7fc1c998))
* docs/billing.md; README API table + features + structure tree. ([67bd75b](https://github.com/akashshindelogistics-lgtm/logistics/commit/67bd75b10f95bc9e7e1e65744fcc62408604f472))
* docs/dispatch-lifecycle.md Returns section; README. ([6306e51](https://github.com/akashshindelogistics-lgtm/logistics/commit/6306e51f442387d96c48278863744cd8dbf296ab))
* docs/vehicle-compliance.md; README API table + features. ([26770be](https://github.com/akashshindelogistics-lgtm/logistics/commit/26770be797c9121f6ca6210436d40bd50d3bf5eb))
* mark periodic-tests 2hr cadence todo as done ([5215608](https://github.com/akashshindelogistics-lgtm/logistics/commit/5215608a857276035b454be04ccd512564c378ba))
* plan for MySQL test database flushing & isolation ([49ddb69](https://github.com/akashshindelogistics-lgtm/logistics/commit/49ddb697800a5fa8227d1bab01b0f7cc722f5b73))
* **readme:** document the headed full-workflow demo test ([4427453](https://github.com/akashshindelogistics-lgtm/logistics/commit/44274533a63b68354fb17799997e155903a3f3e1))
* sync README API table with the current routes ([d63849b](https://github.com/akashshindelogistics-lgtm/logistics/commit/d63849b6f5af231de2deabe7ede6a7f6cd45a7bd))
* **todo:** mark 'Create release for the current state' done ([65a14f4](https://github.com/akashshindelogistics-lgtm/logistics/commit/65a14f490bf50889bf7833f89558b46ba3ddc4fd))
* **todo:** mark 'Create release for the current state' done ([2b09bae](https://github.com/akashshindelogistics-lgtm/logistics/commit/2b09baeefe0bf79ab05f11fbcd0944049473e5ea))
* **todo:** mark 'remove main branch' done ([15fa82f](https://github.com/akashshindelogistics-lgtm/logistics/commit/15fa82f0abae5cd714cae53fe26c99ee20044cec))
* **todo:** mark 'remove main branch' done ([830f299](https://github.com/akashshindelogistics-lgtm/logistics/commit/830f299bbb9c4198871e0217f3676ef99cecaa0b))
* **todo:** mark units-of-measure, godown max_capacity and stock reorder threshold done ([35a0605](https://github.com/akashshindelogistics-lgtm/logistics/commit/35a06057b4ce10882ce7f13a11ad2f5bcc3800ad))

## [0.2.1](https://github.com/akashshindelogistics-lgtm/logistics/compare/v0.2.0...v0.2.1) (2026-09-11)


### Documentation

* **todo:** mark 'Create release for the current state' done ([65a14f4](https://github.com/akashshindelogistics-lgtm/logistics/commit/65a14f490bf50889bf7833f89558b46ba3ddc4fd))
* **todo:** mark 'Create release for the current state' done ([2b09bae](https://github.com/akashshindelogistics-lgtm/logistics/commit/2b09baeefe0bf79ab05f11fbcd0944049473e5ea))

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
