# Freight billing

Tracked by the `todo.org` item *"Add freight billing: a cost per dispatch,
invoice generation, and a payment status (paid/pending/overdue) per
customer."*

## Model

Exactly **one invoice per dispatch**. Raising an invoice records the
dispatch's freight cost and starts the clock on payment.

```
Orgs ──1:N──▶ Dispatches ──1:1──▶ Invoices
                                  (amount, issued_on, due_on, paid_on?)
```

- `Invoice { id, org_id, dispatch_id, customer_id, amount, issued_on,
  due_on, paid_on?, status }` — `src/logistics/billing/invoice.rs`.
  `org_id` and `customer_id` are copied from the dispatch. `dispatch_id` is
  `NOT NULL UNIQUE`, and the table has `ON DELETE CASCADE` foreign keys to
  both `Orgs(id)` and `Dispatches(id)`.
- `amount` is a whole number in the deployment's currency (there is only
  one). It must be greater than zero.
- Dates are ISO `YYYY-MM-DD` strings, validated as real calendar dates on
  write and compared lexicographically (ISO dates sort chronologically).
  `issued_on` is stamped server-side when the invoice is raised; `due_on` is
  supplied by the caller; `paid_on` is stamped when the invoice is paid.
- `status` is **computed on every read**, never stored:
  - `PAID` — `paid_on` is set.
  - `OVERDUE` — unpaid and `due_on` is before today.
  - `PENDING` — unpaid and not yet due.
- `Invoice::customer_summary(customer_id)` aggregates a customer's invoices
  into `{ invoice_count, total_outstanding, overdue_count, invoices }` where
  `total_outstanding` is the summed `amount` of every non-paid invoice.

The date maths uses `days_from_civil` / `civil_from_days` helpers local to
the module (mirroring `vehicle::document`), so the crate takes no date
dependency.

## API

| Method | Path | Description |
|---|---|---|
| POST | `/api/dispatches/{id}/invoice` | Raise an invoice for a dispatch — `{ amount, due_on }`. `400` on a bad amount/date, `409` if the dispatch is already invoiced |
| GET | `/api/dispatches/{id}/invoice` | The dispatch's invoice, or `404` if not yet invoiced |
| PUT | `/api/invoices/{id}` | Amend `amount` / `due_on`. `409` once the invoice is paid |
| POST | `/api/invoices/{id}/pay` | Mark the invoice paid (idempotent — a no-op if already paid) |
| GET | `/api/orgs/{id}/invoices` | Every invoice for the org, most recently issued first |
| GET | `/api/customers/{id}/billing` | The customer's payment standing (outstanding total, overdue count, invoices) |

Ownership is checked the same way as the rest of the system: the
per-dispatch routes go through `load_owned_dispatch`, the by-id routes
through `load_owned_invoice` (checks the invoice's stored `org_id`), and the
customer route through `load_owned_customer`.

## Dashboard

- **Dispatches page** gains a **Billing** column. An uninvoiced dispatch
  shows an "Invoice" button that opens an inline amount + due-date form; an
  invoiced one shows the amount, a colour-coded status badge, and a "Mark
  paid" button while unpaid.
- **Customers page** gains a **Billing** column showing each customer's
  outstanding balance and an "N overdue" badge, computed client-side from a
  single `GET /api/orgs/{id}/invoices` call.
