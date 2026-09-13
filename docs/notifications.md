# Dispatch notifications

When a dispatch is **created**, and again when it's **delivered**, the system
records a notification for each party it should tell:

- **created** → the customer ("your order is on its way") and the assigned
  driver ("new trip assigned")
- **delivered** → the customer ("your order has been delivered")

## What "recording" means

Every notification lands in the `Notifications` table with a status:

| status | meaning |
| --- | --- |
| `QUEUED` | ready to send, but no provider is configured for its channel yet |
| `SKIPPED` | no contact on file for this party, nothing to send |
| `SENT` | delivery succeeded |
| `FAILED` | delivery was attempted and the provider reported an error |

Recording and delivery happen in the same request: right after a `QUEUED`
row is inserted, `Notification::deliver_queued_best_effort` calls
[`crate::logistics::notification::delivery`] to actually send it — Twilio for
SMS, [Resend](https://resend.com) for email (a JSON-over-HTTPS transactional
email API, chosen over raw SMTP so this reuses the same reqwest-based
HTTP-call shape already used for the Anthropic integration rather than
adding a new client/protocol dependency). Both are optional and read their
credentials from the environment:

| Channel | Variables |
| --- | --- |
| SMS (Twilio) | `TWILIO_ACCOUNT_SID`, `TWILIO_AUTH_TOKEN`, `TWILIO_FROM_NUMBER` |
| Email (Resend) | `RESEND_API_KEY`, `RESEND_FROM_EMAIL` |

A channel with no credentials configured behaves exactly as before this
feature existed: the row is left `QUEUED` rather than marked `FAILED` —
nothing was actually attempted, and it's ready to send the moment the
provider is configured. Delivery is best-effort either way: a failure only
updates that notification's own status, it never fails the dispatch
create/deliver request it's attached to.

## Channel choice

- **Customer**: email if `Customer.email` is set, otherwise SMS to
  `Customer.phone`, otherwise `SKIPPED`.
- **Driver**: SMS to the driver's phone (drivers always have one), or
  `SKIPPED` if the vehicle has no active assigned driver.

## Customer contact details

`Customer` gained optional `phone` and `email`, set on
`POST /api/orgs/{id}/customers` (alongside name / address / coordinates) or
cleared/updated with `Customer::set_contact`. A blank string stores as `NULL`.
Fresh databases get the columns in `CREATE TABLE`; a long-lived local one is
patched by `ensure_contact_columns` (probe `information_schema`, `ALTER TABLE
ADD COLUMN`).

## Routes

| Method & path | |
| --- | --- |
| `GET /api/dispatches/{id}/notifications` | Notifications for one dispatch, oldest first (owned dispatch) |
| `GET /api/orgs/{id}/notifications` | The org's 100 most recent notifications, newest first |

Both are reads — any authenticated member of the org can see them.

## Frontend

- The **Customers** create form gained optional **Phone** and **Email** fields.
- The **Dispatches** page gained a **Notifications** column: a 🔔 toggle per
  row that expands a panel listing each recorded notification
  (status · recipient kind · channel → recipient · message). It refetches on
  open, so a delivered notification shows up after the status change.

## Tests

- Rust model (`notification/tests.rs`): created records customer + driver and
  prefers email; SMS fallback then `SKIPPED` with no contact; delivered
  notifies only the customer; `list_by_org` is scoped and newest-first;
  cascade delete with the org; `deliver_queued_best_effort` leaves `QUEUED`
  rows `QUEUED` without provider credentials and never touches a `SKIPPED`
  row. Plus `Customer::set_contact` normalisation.
- Rust delivery (`notification/delivery.rs`): `deliver` returns
  `NotConfigured` for SMS/email without credentials (including a partial
  Twilio credential set) — the only path tested, matching this codebase's
  convention of never hitting a live third-party API in tests (see
  `ai/status.rs`'s equivalent note about the Anthropic call).
- Rust routes (`routes.rs`): creating a dispatch records two notifications
  (customer `SKIPPED`, driver `QUEUED` SMS); delivering records a
  `DISPATCH_DELIVERED` for the customer; the org feed is org-scoped (`403`
  otherwise); `POST /customers` stores contact details.
- Frontend unit: notifications api client; the Customers form passes contact
  details; the Dispatches page loads and shows a dispatch's notifications.
- Playwright (`notifications.spec.ts`): a customer created with an email gets
  it captured, a dispatch records notifications visible on the Dispatches
  page, and the org feed carries them. Full-flow demo has a "the customer and
  driver were notified" step; `npm run test:e2e:demo:notifications` is the
  standalone headed walkthrough.
