# Live database cleanup

Status: **step 1 done, nothing run on live data** (2026-11-04). How to separate
real data from test data in the live database, and stop it mixing again. Do this
before turning on email or agents.

**Decided by the owner on 2026-11-04:** Mauli Brickvision AAC is the only real
organization. Keep the old database for 30 days. The Oracle VM holds no data.

## The problem

The local deployment and local test runs share one database. `logi-api` uses
`mysql://root:...@127.0.0.1:3306/logistics` (from
`~/.config/logistics-ngrok/api.env`), and with no `DATABASE_URL` the backend
defaults to the same `logistics` database on the same server. So anything started
with plain `cargo run` writes to the live data, and that includes the servers the
Playwright tests and demos start.

| Source | Reaches the live database? | Why |
| --- | --- | --- |
| Local Playwright suite and demos | **Yes** | Their API has no `DATABASE_URL` |
| Plain `cargo run` | **Yes** | Same default |
| `cargo test` | No | Each test gets its own `logistics_test_<uuid>` database |
| Frontend CI | No | Its own MySQL service container |

What was seen on 2026-11-03/04: 2,612 organisations, of which about 2,606 match a
rough name pattern for generated data (`Edit Driver ...`, `Demo Logistics Co ...`),
and 180 of 662 drivers with the demo phone number `0`. Those counts are only a
guide: **the pattern isn't accurate enough to choose what to delete.** A real
company whose last word has eight letters would match it.

Why it has to be fixed before going live:

- Email and agents would act on all of it. The delay alert scan and agents cover
  every org, so test customers would get messages, and bounces hurt the Gmail
  account's standing.
- Real customer data sits in a database that test runs freely write to.
- Reports, the org list on the login page and the assistant all include the noise.

## A bug that affects the cleanup

`DELETE /api/orgs/{id}` only deletes the `Orgs` row and relies on foreign keys to
remove the rest. Two tables have no such key:

| Table left behind | What stays | Also left behind through it |
| --- | --- | --- |
| `OrgCredentials` | The org's name and password hash | |
| `Dispatches` | Every dispatch | `DispatchLineItems`, `DispatchStatusHistory`, `DispatchProofOfDelivery` |

So deleting an org through the app doesn't fully delete it, which also matters for
any customer who asks to have their data removed. And files on disk outlive their
`UploadedFiles` rows. The leftover credentials also explain why deleted orgs can
stay on the login page: its list comes from `OrgCredentials`. Step 1 fixes the code,
and `cleanup_db` removes what's already been left behind.

## The approach

**Copy the database and clean the copy, instead of deleting in place.** The current
`logistics` database stays untouched until the new one has proved itself, so going
back only means changing one line and restarting.

**Keep by name, not delete by pattern.** A person names the organization to keep,
and everything else goes.

## Steps

### 1. Stop it happening again (done, no data touched)

- `remove_organization` now also removes the org's dispatches and credentials in one
  transaction, and its files on disk. The missing foreign keys come in step 6, since
  today's orphan rows would block adding them.
- `playwright.demo.config.ts` starts its API with
  `DATABASE_URL=mysql://root:password@localhost:3306/logistics_e2e`, and the README
  shows the same for the headless suite. The backend creates that database by itself.
- This doesn't protect plain `cargo run`. Step 3 does, by moving the live data away
  from the default name.

### 2. Rehearse on a scratch copy

Stop nothing yet. Dump the live database and restore it as
`logistics_cleanup_rehearsal`. The dump contains password hashes and customer data,
so keep it outside the repository with `chmod 600`. Then:

```bash
export DATABASE_URL=mysql://root:PASSWORD@127.0.0.1:3306/logistics_cleanup_rehearsal
cargo run --bin cleanup_db -- --keep "Mauli Brickvision AAC" \
  --uploads-list ~/backups/uploads-to-remove.txt          # reports only
cargo run --bin cleanup_db -- --keep "Mauli Brickvision AAC" \
  --uploads-list ~/backups/uploads-to-remove.txt --apply
```

**Before applying, check the report by eye:** the kept row counts should look like
Mauli's real data, and everything else should be removed. If the name doesn't match
exactly one organization, the tool stops and changes nothing.

What the tool does, in one transaction:

- Refuses to run on a database named `logistics`, or without `DATABASE_URL` set.
- Removes other orgs' rows from every table with an `org_id`, including rows left by
  earlier deletes, then child rows with no parent, then the other orgs.
- Rolls everything back unless these pass: one org left, and every table holding
  exactly the rows the kept org had beforehand.

Log in to the rehearsal copy as Mauli and look around, then drop it.

### 3. Move over

Takes a few minutes, with the API stopped so nothing is written mid-copy.

1. Stop `logi-api`.
2. Take a fresh dump, and restore it as `logistics_live`.
3. Run `cleanup_db` on `logistics_live` as above, reporting first and then applying.
4. Create a MySQL user with access to `logistics_live` only, replacing `root`, so a
   mistake in the app can't reach other databases.
5. Point `DATABASE_URL` in `api.env` at it, using the new user, and start `logi-api`.
6. Log in as a kept org. Check its dispatches, customers and reports, and that the
   login page's org list holds only the kept orgs.

**Going back:** restore the old `DATABASE_URL` line and restart. Anything entered
since the switch would have to be re-entered, so decide within the first day.

### 4. Tidy up afterwards

- Delete only the files on the list `cleanup_db` wrote. **Never empty the uploads folder:**
  it also holds the published Android app (`driver-app/`), which has no database row.
- After 30 days of no problems (from the move), drop the old `logistics` database and let it be
  recreated empty as the development default. Until then it still holds real customer
  data, and a plain `cargo run` still writes to it.
- Keep one encrypted dump for as long as you need, then delete it.
- Add the missing foreign keys now that no orphan rows are left.
- Add a daily dump for this deployment. `deploy/README.md` only describes one for the
  Oracle VM.

## Not covered

- **Agent tables.** Not deployed yet. `cleanup_db` covers them anyway, as it finds
  every table with an `org_id`, and they're removed with their org by foreign key.
