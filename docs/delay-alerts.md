# Delay / ETA-drift alerts

Tracked by the `todo.org` item *"Delay/ETA-drift alerts - notify when an
in-transit dispatch is running late against its planned schedule."*

## Model

There is still no per-dispatch promised-delivery-date field (the same gap
`docs/reports.md`'s on-time rate flags as a known limitation) — adding one
would mean threading a new caller-supplied field through every dispatch- and
trip-creation call site for a value nobody has asked to customize yet. So
this reuses the same fleet-wide target `OpsReport`'s on-time rate already
assumes:

- `PROMISED_DELIVERY_HOURS: f64 = 72.0` (`src/logistics/dispatch/dispatch.rs`)
  — a dispatch is expected to reach `DELIVERED` within 72 hours of being
  created. Not stored on the row; computed from `dispatched_at` on every
  check, the same way `reports::ON_TIME_TARGET_HOURS` is (a separate, ==-valued
  constant — the two aren't wired together, so changing one doesn't silently
  change the other).
- `DispatchOrder::is_running_late(&self) -> bool` — `true` when the dispatch
  is still `IN_TRANSIT` and more than `PROMISED_DELIVERY_HOURS` have passed
  since `dispatched_at`. A dispatch that has already reached a terminal
  status (`DELIVERED`/`RETURNED`/`CANCELLED`) is never "late", however old —
  there's nothing left to alert anyone about.
- `DispatchOrder::hours_late(&self) -> Option<i64>` — how many whole hours
  past the target, for a human-readable alert body; `None` when not late.
- `DispatchOrder::list_in_transit()` — every `IN_TRANSIT` dispatch across
  every org, the candidate set the scan below filters down.

## Alerting

`src/logistics/notification/delay_alerts.rs`'s `scan_and_alert()`:

1. Loads every `IN_TRANSIT` dispatch (`DispatchOrder::list_in_transit`) and
   keeps the ones `is_running_late()`.
2. For each, checks whether it already carries a `DISPATCH_RUNNING_LATE`
   notification (`Notification::list_by_dispatch`) — if so, skips it. This is
   what makes the scan idempotent: it can run as often as you like (or be
   called twice at once) without ever double-sending.
3. Otherwise looks up the dispatch's customer and records + attempts to
   deliver a new `NotificationEvent::DispatchRunningLate` notification via
   `Notification::record_dispatch_running_late`, reusing the existing
   Twilio/Resend delivery pipeline (`notification::delivery`) exactly as the
   dispatch-created / dispatch-delivered events do — this is a new trigger
   condition on the same mechanism, not a new one. The body is
   AI-personalized via `ai::notification_copy::generate_dispatch_running_late_customer_body`
   when `ANTHROPIC_API_KEY` is configured, falling back to a fixed template
   on any error, matching every other notification in this codebase.
4. An error on one dispatch (its customer was since deleted, say) is logged
   and skipped rather than aborting the rest of the scan.

`main.rs` spawns this on a `tokio` timer at startup (`DELAY_ALERT_SCAN_INTERVAL`,
currently 15 minutes) for the lifetime of the process — this app's actual
deploy target is a persistent VM (see `docs/file-uploads.md`), not an
ephemeral-per-request platform, so an in-process background loop needs no
new infrastructure. There is no user-facing route for it; nothing calls it
except that loop and the tests below.

## Frontend

No new API call. `frontend/src/lib/dispatchLifecycle.ts`'s `isRunningLate()`
mirrors the backend check against the same `PROMISED_DELIVERY_HOURS`
constant, computed client-side from the `status` / `dispatched_at` a
dispatch's own row already carries. The Dispatches page shows a red
"⚠ Running late" badge next to the status tag for a matching row — purely a
visual cue; the customer-facing alert itself is the backend's SMS/email.

## Known limitations

- Still one fleet-wide 72h target, not a per-dispatch promised date — see
  "Model" above.
- No Playwright coverage of the "Running late" badge itself:
  `dispatched_at` is stamped by the server at creation and isn't something
  the public API lets a caller backdate, so there's no way to make a real
  dispatch overrun its window from a browser-driven e2e test without adding
  a test-only backdoor. Covered instead by a frontend unit test
  (`Dispatches.test.tsx`, mocking an old `dispatched_at`) and a Rust
  integration test for the scan itself (`delay_alerts.rs`, which backdates a
  real dispatch's `dispatched_at` via a direct SQL update, the same
  technique `reports/tests.rs` uses for its on-time-rate tests).
- The scan loop isn't itself covered by a test — only the `scan_and_alert()`
  function it calls on a timer, called directly. Testing the timer would mean
  either waiting 15 real minutes or adding test-only timing controls for a
  three-line loop; not worth it.

## Tests

- Rust unit (`dispatch.rs`): `is_running_late` / `hours_late` true/false
  across `IN_TRANSIT` past and within the window, and every terminal status
  regardless of age.
- Rust integration (`delay_alerts.rs`): a real, backdated `IN_TRANSIT`
  dispatch gets exactly one alert; calling the scan again never double-sends;
  a dispatch still within its window, or no longer `IN_TRANSIT`, is skipped.
- Rust (`ai/notification_copy.rs`): the new prompt builder includes the
  customer name, order reference and hours-late figure; the generator's
  missing-API-key fallback path.
- Frontend unit (`dispatchLifecycle.test.ts`): `isRunningLate` across the
  same cases as the Rust unit tests. (`Dispatches.test.tsx`): the badge
  renders for an overdue `IN_TRANSIT` order and not for a recent one or an
  old terminal one.
