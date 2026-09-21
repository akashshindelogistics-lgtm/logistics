# New-organisation onboarding (plan)

Status: **planned, not implemented.**

Today a new org registers with a name, address and password, is logged in, and
lands on its detail page (`frontend/src/pages/Register.tsx`). Nothing tells
them what to set up next, and a dispatch needs several records to exist first.
This plan adds a guided onboarding flow.

## Decisions taken (defaults)

These were open questions; the defaults below are what this plan assumes.

| Question | Decision |
| --- | --- |
| Who can sign up? | Public, self-serve. |
| Who owns the account? | The first **Admin `OrgUser`** with an email + password, created at signup. Login moves to the existing `POST /api/auth/user-login` (email is globally unique, see [roles.md](roles.md)). |
| How is progress tracked? | A checklist **derived from real data**, not a stored wizard state. |
| Bulk import (CSV)? | In scope, but as a second phase. |
| Invite teammates / drivers? | An optional, skippable step. |

## Problems in the current signup

- **No email or phone.** Nothing to verify, nothing for password reset or
  notifications.
- **Org-level shared password.** `POST /api/auth/login` picks an org by name
  and password. Everyone shares one credential and roles cannot apply.
- **Tenant names are public.** `GET /api/auth/orgs` lists every org so the
  login form can offer a dropdown. That should stop once login is by email.
- **No guidance.** A new org sees empty pages and has to know that dispatch
  needs a godown with stock, a vehicle with an active assigned driver, and a
  customer with a location.
- **No abuse protection** on a public endpoint that creates a tenant.

## Design

### 1. Signup

`POST /api/auth/signup` (public) replaces the name+password `POST /api/orgs`
for the browser flow. The old route stays for API clients and tests.

Body: `{ org_name, address, owner_name, email, password }`.

- Creates the org and its first `OrgUser` with role `Admin` in one
  transaction, so a failure cannot leave an org with no owner.
- Email is lowercased and must be unique (`409` otherwise, as for team
  members). Password minimum length is enforced.
- Returns a user-scoped token, same shape as `user-login`, so the new owner
  is signed in immediately.
- **Idempotency:** a repeated submit with the same email gets `409`, never a
  second org.
- **Abuse:** per-IP rate limit on signup, and a hook for a CAPTCHA token field
  (validated only when a secret is configured).
- **Verification:** the account works straight away, but `OrgUser` gains
  `email_verified_at`. Until verified, the dashboard shows a banner and
  outbound notification emails are held. `POST /api/auth/verify-email
  {token}` and `POST /api/auth/resend-verification` complete the loop. Sending
  needs a mail transport, which is a new dependency and config (see Open
  questions).
- **Schema:** new columns are added with the same `ensure_*_column`
  patch-on-read pattern used for `tracker_key`, since there is no formal
  migration system.

### 2. Login

- The login page defaults to email + password (`user-login`).
- `GET /api/auth/orgs` and the org-name login remain for now so existing orgs
  keep working, but are marked deprecated in the OpenAPI spec and hidden from
  the UI. Removing them is a later, separate change because it breaks
  existing orgs that have no users.
- Existing orgs get a one-time prompt to create their first Admin user with an
  email. Suggested: an "Upgrade your account" banner on the org-level session.

### 3. The setup checklist

`GET /api/orgs/{id}/onboarding` returns a computed status; nothing about step
completion is stored.

```json
{ "steps": [
  { "key": "org_location",  "required": true,  "done": true  },
  { "key": "godown",        "required": true,  "done": false },
  { "key": "stock",         "required": true,  "done": false },
  { "key": "vehicle",       "required": true,  "done": false },
  { "key": "driver",        "required": true,  "done": false },
  { "key": "driver_assigned","required": true, "done": false },
  { "key": "customer",      "required": true,  "done": false },
  { "key": "first_dispatch","required": false, "done": false },
  { "key": "invite_team",   "required": false, "done": false },
  { "key": "gps_tracker",   "required": false, "done": false }
], "dismissed": false }
```

`done` comes from queries the app already has (counts of godowns, stock rows,
vehicles, drivers, a vehicle with an assigned active driver, customers with a
location, dispatches, org users beyond the owner, vehicles with a recent
location). Because it is derived, data added through the API or CSV import
ticks the checklist too, and it can never disagree with reality.

The only stored state is `Organization.onboarding_dismissed` (bool), set by
`PUT /api/orgs/{id}/onboarding/dismiss`, so a user can hide the checklist.

The steps encode the real dependency order: godown -> stock, and vehicle ->
driver -> assignment, because `dispatch_stock_to_customer` only considers
vehicles joined to an active driver and with enough capacity.

### 4. Frontend

- **Signup page** gains owner name and email; a confirm-password field stays.
- **Onboarding panel** on the dashboard and org detail page while incomplete
  and not dismissed: progress bar, each step with a button that deep-links to
  the right form (for example "Add a vehicle" scrolls to or opens that form).
  Required steps are listed first; optional ones are visually secondary.
- **Empty states** on Godowns, Vehicles, Drivers, Customers and Dispatches
  point at the next unfinished step instead of showing a blank table.
- **Address geocoding (nice to have):** the org, godown and customer forms take
  an address and fill latitude/longitude, instead of asking users to type
  coordinates. Provider choice and rate limits are an open question.
- **Resumable:** because state is derived server-side, a user can log out and
  return on another device and see the same progress.
- **Verify banner** for unverified email, with a resend button.

### 5. CSV import (phase 2)

`POST /api/orgs/{id}/import/{kind}` where kind is `vehicles`, `drivers`,
`customers` or `stock`. Multipart CSV upload, reusing the upload plumbing from
[file-uploads.md](file-uploads.md).

- A dry-run mode (`?dry_run=true`) returns per-row errors without writing.
- Real import is all-or-nothing per file, in one transaction, so a bad row
  does not leave half a fleet.
- Row limit (suggested 1,000) and file-size limit.
- A downloadable template per kind, so column names are not guesswork.
- Stock rows reference a godown by name; vehicles may reference a driver by
  licence number; both are resolved server-side with clear errors.

### 6. Optional: sample data

A "Load sample data" action that creates a demo godown, stock, vehicle, driver
and customer, all tagged so a "Remove sample data" action deletes exactly
those. This lets a new user complete a dispatch in a minute. It is an add-on,
not part of the first phase, and the tagging must be a real column, not a name
convention.

## Security and tenancy

- Every new org-scoped route uses the existing `AuthenticatedOrg` extractor and
  checks the path org id against the token, as other routes do.
- The onboarding, import and dismiss routes are Admin-only where they write
  (`require_role`). The status route is readable by any member.
- Signup and verification are the only new unauthenticated writes. Tokens for
  verification are single-use, high-entropy and expire.
- Passwords use the same hashing as the existing login.
- Never log passwords, verification tokens or CAPTCHA tokens.

## Compliance

- Accept terms and privacy policy at signup (`terms_accepted_at` stored).
- If [driver phone tracking](driver-phone-tracking.md) ships, driver consent is
  collected at device pairing, not at org signup.

## Phases

1. **Backend signup and login:** `POST /api/auth/signup` (org + Admin user in
   one transaction), password rules, rate limit, duplicate handling,
   `terms_accepted_at`. Unit tests, route tests, OpenAPI, README route table.
2. **Checklist API and UI:** `GET /api/orgs/{id}/onboarding`, dismiss, the
   dashboard panel, empty states, updated Register and Login pages. Frontend
   unit tests, a Playwright spec, and a standalone headed demo flow
   (`onboarding.demo.ts`). Update the full-flow demo to sign up via the new
   path.
3. **Email verification and existing-org upgrade:** mail transport, verify and
   resend routes, banner, and the "create your first Admin user" prompt for
   pre-existing orgs.
4. **CSV import:** the four import kinds, dry run, templates, UI.
5. **Optional:** sample data, address geocoding.

## Tests

- Rust model and routes: signup creates org + Admin atomically; duplicate
  email `409` and no orphan org; weak password `400`; rate limit; checklist
  flips each step as the matching record is created and never regresses;
  dismiss persists; a non-Admin cannot dismiss or import; cross-org access
  `403`.
- Frontend unit: signup form validation and error surfacing; checklist
  rendering for each state; empty-state links.
- Playwright: sign up -> checklist shows all required steps undone -> add a
  godown, stock, vehicle, driver, assignment and customer -> required steps
  complete -> first dispatch succeeds. Plus a resume test (log out, log in,
  progress intact).

## Open questions

- **Mail transport:** SMTP vs a provider API, and where the credentials live.
  Until decided, phase 3 can log the verification link in development.
- **Geocoding provider** and its rate/usage terms.
- **Existing orgs:** how long to keep the org-name login before removing it.
- **Plan limits:** whether a trial or free tier with caps belongs in this
  change or a separate one.
- **CAPTCHA provider**, if any.
