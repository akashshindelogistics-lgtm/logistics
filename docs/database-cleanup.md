# Live database cleanup

Status: **plan, nothing run** (2026-11-04). How to separate real data from test
data in the live database, and stop it mixing again. Do this before turning on
email or agents.

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
`UploadedFiles` rows. The cleanup below handles all three by hand, and step 1 fixes
the code.

## The approach

**Copy the database and clean the copy, instead of deleting in place.** The current
`logistics` database stays untouched until the new one has proved itself, so going
back only means changing one line and restarting.

**Keep by name, not delete by pattern.** A person lists the organisations to keep,
and everything else goes.

## Steps

### 1. Stop it happening again (code, no data touched)

- Backend: `remove_organization` also removes the org's dispatches and credentials,
  in one transaction, with tests. Or add the missing foreign keys, but existing
  orphan rows would block that until the cleanup is done, so it comes later.
- Frontend: `playwright.demo.config.ts` starts its API with
  `DATABASE_URL=mysql://root:password@localhost:3306/logistics_e2e`, and the README
  shows the same for the headless suite. The backend creates that database by itself.
- This doesn't protect plain `cargo run`. Step 5 does, by moving the live data away
  from the default name.

### 2. List the organisations (read only)

Run by the owner, or by Claude with explicit permission, since this reads live data:

```sql
SELECT o.id, o.name, o.address,
       (SELECT COUNT(*) FROM Customers c WHERE c.org_id = o.id)  AS customers,
       (SELECT COUNT(*) FROM Dispatches d WHERE d.org_id = o.id) AS dispatches,
       (SELECT COUNT(*) FROM OrgUsers u WHERE u.org_id = o.id)   AS users,
       (SELECT FROM_UNIXTIME(MAX(d.dispatched_at)) FROM Dispatches d WHERE d.org_id = o.id) AS last_dispatch
FROM Orgs o
ORDER BY dispatches DESC, o.name;
```

Save the result outside the repository: it contains real company names. Mauli
Brickvision is expected to be on the keep list.

### 3. Choose what to keep (owner's decision)

Write the IDs to keep to `~/.config/logistics-ngrok/keep-orgs.txt`, one per line.
The script refuses to run if that file is empty or lists an unknown ID.

### 4. Rehearse on a scratch copy

Dump the live database and restore it as `logistics_cleanup_rehearsal`. Run the
script below there, then run the checks. Repeat until they pass, then drop the copy.
The dump contains password hashes and customer data, so keep it outside the
repository with `chmod 600`.

```sql
-- keep_orgs(id) is loaded from keep-orgs.txt.

-- Files to remove later. Save this list first.
SELECT storage_path FROM UploadedFiles
WHERE org_id NOT IN (SELECT id FROM keep_orgs);

START TRANSACTION;
-- These two have no foreign key to Orgs. Also clears rows left by earlier deletes.
DELETE FROM Dispatches     WHERE org_id NOT IN (SELECT id FROM keep_orgs);
DELETE FROM OrgCredentials WHERE org_id NOT IN (SELECT id FROM keep_orgs);
-- Everything else follows through ON DELETE CASCADE.
DELETE FROM Orgs           WHERE id     NOT IN (SELECT id FROM keep_orgs);
COMMIT;
```

Checks, all of which must pass:

- `SELECT COUNT(*) FROM Orgs` equals the number of lines in the keep list.
- Zero orphans in every table with an `org_id`, by
  `WHERE org_id NOT IN (SELECT id FROM Orgs)`, and zero child rows pointing at a
  missing dispatch in `DispatchLineItems`, `DispatchStatusHistory` and
  `DispatchProofOfDelivery`.
- For each kept org, the customer, dispatch, vehicle, driver and invoice counts match
  step 2 exactly.

### 5. Move over

Takes a few minutes, with the API stopped so nothing is written mid-copy.

1. Stop `logi-api`.
2. Take a fresh dump, and restore it as `logistics_live`.
3. Run the script and the checks on `logistics_live`.
4. Create a MySQL user with access to `logistics_live` only, replacing `root`, so a
   mistake in the app can't reach other databases.
5. Point `DATABASE_URL` in `api.env` at it, using the new user, and start `logi-api`.
6. Log in as a kept org. Check its dispatches, customers and reports, and that the
   login page's org list holds only the kept orgs.

**Going back:** restore the old `DATABASE_URL` line and restart. Anything entered
since the switch would have to be re-entered, so decide within the first day.

### 6. Tidy up afterwards

- Delete only the files on the list saved in step 4. **Never empty the uploads folder:**
  it also holds the published Android app (`driver-app/`), which has no database row.
- After 30 days of no problems, drop the old `logistics` database and let it be
  recreated empty as the development default. Until then it still holds real customer
  data, and a plain `cargo run` still writes to it.
- Keep one encrypted dump for as long as you need, then delete it.
- Add the missing foreign keys now that no orphan rows are left.
- Add a daily dump for this deployment. `deploy/README.md` only describes one for the
  Oracle VM.

## Not covered

- **The Oracle VM deployment.** `todo.org` lists it as unfinished. If it holds data,
  it needs its own review, but it has no test traffic since tests only run locally.
- **Agent tables.** Not deployed yet. They're removed with their org by foreign key.

## Questions for the owner

1. Which organisations are real? (step 3)
2. Is 30 days right for keeping the old database?
3. Does the Oracle VM hold any data?
