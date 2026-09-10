# Dispatch notifications

When a dispatch is **created**, and again when it's **delivered**, the system
records a notification for each party it should tell:

- **created** → the customer ("your order is on its way") and the assigned
  driver ("new trip assigned")
- **delivered** → the customer ("your order has been delivered")

## What "recording" means

Recording is all this feature does today. Every notification lands in the
`Notifications` table with a status:

| status | meaning |
| --- | --- |
| `QUEUED` | a message ready to send — we have a phone or email for this party |
| `SKIPPED` | no contact on file for this party, nothing to send |

Wiring an actual SMS / email provider is a deployment concern: a sender reads
the `QUEUED` rows, delivers them, and flips them to `SENT` / `FAILED`. Nothing
in this repo talks to Twilio or an SMTP server.

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
  cascade delete with the org. Plus `Customer::set_contact` normalisation.
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
