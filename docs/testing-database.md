# Plan: MySQL test database flushing & isolation

Status: **all three phases done** — tracked by the `[#B]` item in `todo.org`
("Need to plan for MySql test database flushing"). The problem statement and
options below describe the state before Phase 1; see each phase's section
for what actually shipped.

## Problem

Every test in the crate talks to a **real MySQL server**, and they all talk to
the **same database**:

- `src/logistics/db/connection.rs` holds a process-global
  `static DB_POOL: OnceLock<Pool>`. The first call to
  `DbConnection::get_connection()` fixes the host / port / database / credentials
  for the entire process; later `DbConnection::new(...)` values are ignored.
- Every call site is hardcoded to
  `DbConnection::new("localhost", 3306, "logistics", "root", "password")`
  (77 test functions across `orgs`, `vehicle`, `stock`, `customer`, `dispatch`,
  `auth`, and the `routes` integration tests).
- The CI workflow `periodic-tests.yml` exports `MYSQL_HOST`, `MYSQL_PORT`,
  `MYSQL_USER`, `MYSQL_PASSWORD`, `MYSQL_DATABASE` — **none of which the code
  reads**. CI passes today only because the service container happens to listen
  on `localhost:3306` with `root` / `password` and a database literally named
  `logistics`.

Consequences:

1. **No flushing.** Rows accumulate across local runs. Tests that create fixed
   names ("Login Test Org", "Me Endpoint Org", …) leave them behind. Count-style
   assertions are all written defensively as `>= 1`, which hides real
   regressions (a broken "list" endpoint that returns stale rows still passes).
2. **Cross-test bleed under parallelism.** `cargo test` runs tests on multiple
   threads in one process against one shared set of tables. Any test that
   asserts an exact count, "first row", ordering, or "table is empty" is
   inherently flaky and can only be made to pass by weakening the assertion.
3. **Local dev DB is clobbered.** Running `cargo test` writes test junk straight
   into the same `logistics` database a developer uses for `cargo run`.
4. **A warm local DB masks CI-only failures** — exactly what bit us in commit
   `1da8c25` (lazily-created `Vehicle` / `Stock` tables). CI provisions an empty
   database each run; local dev never does.
5. **Schema DDL is duplicated.** `CREATE TABLE IF NOT EXISTS` statements for the
   six tables (`Orgs`, `Vehicle`, `Stock`, `Customers`, `OrgCredentials`,
   `Dispatches`) are copy-pasted across ~10 sites. There is no single place to
   provision or migrate a database, which is a prerequisite for any
   flush-and-recreate strategy.

CI is only "flushed" as a side effect of the container being thrown away. The
gaps are **local runs** and **isolation between tests in a single run**.

## Options considered

| Option | Isolation | Keeps parallelism | Code change | Verdict |
|---|---|---|---|---|
| A. `TRUNCATE` all tables before each test | none (races) | no | small | insufficient alone |
| B. Serialize DB tests + `TRUNCATE` between each | full | no | small–medium | **adopt now (Phase 1)** |
| C. Database-per-test (`logistics_test_<uuid>`, drop after) | full | yes | medium (needs pool refactor) | **target (Phase 3)** |
| D. Wrap each test in a transaction, roll back | full | yes | large | not feasible — handlers open many independent pooled connections; a transaction can't span them |
| E. Unique data per test (namespacing, no flush) | partial | yes | medium (touch every test) | complements, not a substitute — DB still grows |

## Recommended plan (phased)

### Phase 1 — deterministic tests, isolated test database — **DONE**

Implemented in `src/logistics/test_support.rs` (`migrate`, `reset_database`),
a `#[cfg(test)]` switch in `DbConnection` that targets `logistics_test`, and
`#[serial(db)]` + `reset_database()` on all 71 DB-touching tests (5 pure
unit tests and the trivial `connection.rs` smoke test were left alone).

While doing this we found `src/main.rs` re-declared `mod logistics;` instead of
using the library crate, so `cargo test` compiled the whole module tree twice
and ran the suite in two processes concurrently against the same database —
`#[serial]` cannot serialize across processes. `main.rs` now uses
`logistics_system::logistics::…`, so only the `lib` test binary has tests.

Original plan for reference:

1. **Add a `test_support` module** (compiled under `#[cfg(test)]`), exposing:
   - `migrate(conn)` — the single source of truth for the schema (moved from the
     scattered `CREATE TABLE IF NOT EXISTS` blocks; production code calls the
     same function on startup instead of creating tables ad hoc).
   - `reset_database()` — `SET FOREIGN_KEY_CHECKS = 0`, `TRUNCATE` every table,
     `SET FOREIGN_KEY_CHECKS = 1`. Runs `migrate` first so a fresh DB works.
2. **Use a dedicated database for tests** — default the test connection to
   `logistics_test` (never `logistics`), created on demand. A developer's
   `cargo run` database is then untouched by `cargo test`.
3. **Serialize DB-touching tests.** Add the `serial_test` crate; mark every test
   that hits MySQL `#[serial(db)]` and call `reset_database()` as its first
   line. Pure-unit tests (JWT, haversine, payload (de)serialization) stay
   parallel and untouched.
4. **Fix CI** (`periodic-tests.yml`): add a step that creates `logistics_test`,
   and pass the DB name through (see Phase 2). Fresh container still applies, so
   this is mostly future-proofing plus making `cargo test` locally match CI.

Outcome: exact-count assertions become possible; `>= 1` checks can be tightened
in a follow-up; local runs stop polluting the dev database.

Cost: ~77 test functions get a `#[serial(db)]` attribute and a
`reset_database();` first line. Mechanical, reviewable in one pass. Test wall
time goes up (serial), acceptable at this suite size.

### Phase 2 — make the connection configurable — **DONE**

Implemented in `src/logistics/db/connection.rs`. `DbConfig::from_env()` reads
`DATABASE_URL` (a full `mysql://user:pass@host:port/db` string) if set,
otherwise `MYSQL_HOST` / `MYSQL_PORT` / `MYSQL_USER` / `MYSQL_PASSWORD` /
`MYSQL_DATABASE` individually, each falling back to the original hardcoded
defaults (`localhost:3306`, `root`/`password`, `logistics`) when unset. Under
`cfg(test)` the database-name default is `logistics_test` instead of
`logistics`, so `cargo test` still never touches a developer's dev database
when no env vars are set — replacing the old `cfg!(test)`-suffix trick that
lived inside `DbConnection::get_connection()`.

All 47 call sites across the domain modules and `test_support.rs` that used
to hardcode `DbConnection::new("localhost", 3306, "logistics", "root",
"password")` now call `DbConnection::from_env()`. `DbConnection` itself stays
the thin pool wrapper the plan called for; `DbConnection::new(...)` remains
available for a caller that genuinely needs a specific, non-default
database (as `DbConfig::connection()` uses internally).

This also fixed the latent CI footgun: `periodic-tests.yml` had exported
`MYSQL_HOST` / `MYSQL_PORT` / `MYSQL_USER` / `MYSQL_PASSWORD` /
`MYSQL_DATABASE` since Phase 1 landed, but nothing in the code read them —
CI passed only because the service container happened to match the
hardcoded defaults. Verified locally by pointing `MYSQL_DATABASE` and,
separately, `DATABASE_URL` at scratch databases and confirming
`cargo test` connects to and migrates each one.

### Phase 3 — database-per-test, restore parallelism — **DONE**

Implemented as planned, with one addition the plan didn't anticipate:

1. `src/logistics/db/connection.rs`'s single `static DB_POOL: OnceLock<Pool>`
   became `static POOLS: OnceLock<Mutex<HashMap<String, Pool>>>`, keyed by
   database name — `DbConnection::get_connection()` fetches or creates the
   pool for `self.db_name` instead of every call site sharing one pool bound
   to whichever database happened to initialize it first.
2. Rather than passing a `DbConfig` through every function (it would have
   touched every call site across the domain modules a second time), a
   `#[cfg(test)]` thread-local override does the job: `TestDb::create()`
   (`src/logistics/test_support.rs`) generates a `logistics_test_<uuid>`
   name, calls `connection::set_test_db_override(Some(name))`, migrates the
   new database, and returns a guard. Every `DbConnection::from_env()` call
   made on that thread afterwards resolves to it. This is safe under
   `cargo test`'s default parallelism because libtest gives each `#[test]`
   its own OS thread, and `#[actix_web::test]` runs its whole async body on
   a single-threaded executor pinned to that same thread (`actix_rt::System
   ::new().block_on(...)`), so the override holds across every `.await`
   too — confirmed empirically, not just by reading the executor source.
   `Drop for TestDb` drops the database, evicts its pool from the registry,
   and clears the override.
3. All ~90 tests that called `reset_database()` under `#[serial_test::serial
   (db)]` now start with `let _db = TestDb::create();` instead, with no
   `#[serial]` — `cargo test` runs at full parallelism. The `serial_test`
   dev-dependency was removed as a result.
4. Not implemented: an automated cleanup step for `logistics_test_%`
   databases orphaned by a crashed run (a hard process abort, not a normal
   panic — `Drop` still runs on a panic unwind, so an assertion failure
   alone does not leak one). Left as manual (`DROP DATABASE`) since the
   plan's own local/CI cleanup step turned out to be unnecessary for the
   common case.

**The addition the plan missed:** minting one `Pool` per test (up to
`nproc` of them at once, each with the `mysql` crate's default `min: 10,
max: 100` connection bounds) exceeded MySQL's default `max_connections`
(151) well before 20 tests were even running concurrently, which surfaced
as a `Mutex` poisoned by one test's connection failure aborting *every*
other test whose `TestDb::drop()` touched the registry afterwards — a
poisoned-lock panic inside a `Drop` that runs during an unrelated panic's
unwind is a double panic, which Rust turns into a process abort rather than
a normal test failure. Fixed by (a) capping pool size to `(1, 4)` under
`cfg(test)` — each test only ever needs one or two connections at a time —
and (b) having the registry's lock recover from poisoning
(`.unwrap_or_else(|poisoned| poisoned.into_inner())`) instead of
propagating it, since the only state behind that lock is a `HashMap` that
tolerates a torn insert/remove just fine.

Verified against a live MySQL: `cargo test --all` — 106 tests, 0 failures,
in ~11-14s at default (`nproc`-wide) parallelism, down from ~50-55s
serialized in Phase 1/2; repeated across three consecutive full runs with
no flakiness; confirmed no `logistics_test_%` databases or extra
`Threads_connected` were left behind after a clean run.

## Immediate next step

Implement **Phase 1**. It is self-contained, unblocks tighter assertions, and
is a prerequisite for Phases 2–3. Phases 2 and 3 can follow as separate PRs.
