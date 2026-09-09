# Role-scoped user accounts

Until now an organisation had a single shared password (`OrgCredentials`,
keyed by `org_id`). That login still works and is treated as the org's
**Admin**. On top of it an Admin can add named team members who each sign in
with their own email + password and carry one of three roles.

## Roles

| Role | Can write |
| --- | --- |
| **Admin** | Everything, including managing team members and deleting the org. The org-owner login is always Admin. |
| **Dispatcher** | Run and advance dispatches, raise/amend/pay invoices; manage customers, drivers, vehicles. |
| **WarehouseStaff** | Create/update/delete/transfer godown stock; manage godowns. |

**Every role can read everything in its own org** — roles only gate writes.

## Model

- `OrgRole` enum (`ADMIN` / `DISPATCHER` / `WAREHOUSE_STAFF`). `OrgRole::from_str`
  falls back to `WarehouseStaff` (least privilege) for anything it doesn't
  recognise, so a bad stored/claimed value can never widen access.
- `OrgUser { id, org_id, name, email (unique, lowercased), role, is_active }`
  in an `OrgUsers` table (cascade-deleted with the org). `password_hash` is
  bcrypt and never serialised.
- `OrgUser::verify_login(email, password)` returns the user only when the
  password matches **and** `is_active`.

## Auth plumbing

- JWT `Claims` gained `user_id: Option<String>` and `role: String`. Both have
  serde defaults, so tokens issued before this change still decode — as an
  Admin org-owner token.
- `POST /api/auth/user-login { email, password }` issues a role-scoped token
  (email is globally unique, so no org id is needed).
- The `AuthenticatedOrg` extractor now carries `role` and `user_id`.
  `AuthenticatedOrg::require_role(&[...])` returns a `403` when the caller's
  role isn't allowed. It's applied to: user management + `DELETE /api/orgs/{id}`
  (Admin); `dispatch` + dispatch-status + invoice create/amend/pay
  (Admin | Dispatcher); godown + stock writes (Admin | WarehouseStaff).
  Reads and the remaining routes stay org-scoped only.

## User management routes (Admin only)

| Method & path | |
| --- | --- |
| `GET /api/orgs/{id}/users` | List the org's team members |
| `POST /api/orgs/{id}/users` | Add one — `409` on a duplicate email, `400` on a bad email or a password under 8 chars |
| `PUT /api/users/{id}` | Change name / role / active flag |
| `DELETE /api/users/{id}` | Remove — `400` if you target your own account |

## Frontend

- The Login page has an **Organization / Team member** tab switch; team-member
  mode asks for an email instead of the org dropdown.
- `storeAuth` keeps `role` and `user_name`; `isAdmin()` gates UI.
- A **Team** page (`/team`, sidebar link shown only to Admins) lists members
  and adds/edits/removes them, with inline role and active-status controls.
  Non-admins who navigate there get an "Admins only" notice.
- Role-gating the rest of the dashboard UI (hiding the dispatch/stock forms
  for the wrong role) is left as polish — the backend `403` is the guarantee.

## Not done here (follow-ups)

- Registration still creates only the org-owner credential; there's no
  self-serve invite flow or per-user password reset.
- Only the ~8 highest-impact write routes are role-gated; the rest remain
  org-scoped for any authenticated member.

## Tests

- Rust model (`user/tests.rs`): role parsing incl. least-privilege fallback;
  create → authenticate → reload; email uniqueness across orgs; input
  validation; inactive users can't log in; org-scoped listing; cascade delete.
- Rust routes (`routes.rs`): user-login rejects wrong password / inactive;
  only Admin manages users and duplicate email is `409`; a Dispatcher can
  dispatch but WarehouseStaff can't (and vice-versa for stock);
  `DELETE /api/orgs/{id}` needs Admin.
- Frontend unit: users api client; auth role storage + `userLogin`; the Team
  page (list, add, role change, non-admin notice, error surfacing); Login
  team-member mode; the Sidebar Team link is Admin-only.
- Playwright (`users.spec.ts`): an Admin adds a member on the Team page and
  they sign in by email; a non-admin is bounced from `/team`; the role gates
  hold over the API.
