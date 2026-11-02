# Splitting the monorepo into three repos (plan)

Status: **planned** (2026-11-02). Tracked by the top-level item in `todo.org`
("devide single mono repo into multiple repos").

Today one repo, `akashshindelogistics-lgtm/logistics`, holds the Rust backend
(`src/`, `Cargo.toml`), the React dashboard (`frontend/`), the native Android
driver app (`android/`) and the Expo app being retired (`mobile/`). They share
one version, one release tag and one set of CI workflows. This plan gives each
part its own repo, CI, README and release tags.

## Decisions (confirmed with the user)

| Question | Decision |
| --- | --- |
| The existing repo | Becomes the **backend** repo and is renamed `logistics-backend`. It keeps PRs #1 onwards, issues, tags v0.1.0 to v0.14.x, releases, the `logistics-api` GHCR image, Actions variables and secrets. GitHub redirects the old URL and git remotes. |
| New repos | `logistics-frontend` and `logistics-android`, public like today. |
| History | Kept per folder: `git subtree split` (built into git) moves each folder's commits to the new repo's root, so `git log` and `blame` still work there. Tags aren't carried over. |
| `mobile/` (Expo) | Stays in the backend repo until phase 6 of [android-driver-app.md](https://github.com/akashshindelogistics-lgtm/logistics-android/blob/master/docs/android-driver-app.md) deletes it. |

## What goes where

| Repo | Contents | From today's paths |
| --- | --- | --- |
| `logistics-backend` (renamed) | Rust API, `gen_openapi`, Dockerfile, `deploy/`, feature docs, `todo.org`, `mobile/` until retired | everything except `frontend/` and `android/` |
| `logistics-frontend` (new) | React dashboard, Vitest, Playwright specs and demos | `frontend/**` → repo root |
| `logistics-android` (new) | Kotlin driver app, its scripts and docs | `android/**` → repo root; `docs/android-driver-app.md` and `docs/android-driver-install.md` → `docs/` |

Feature docs that describe the API and the dashboard together (`dispatch-lifecycle.md`,
`driver-phone-tracking.md`, ...) stay in the backend repo. Their mentions of
`frontend/...` and `android/...` paths become links to the new repos.

## CI per repo

**Backend** keeps:
- `periodic-tests.yml` (cargo tests)
- `rust-lint.yml`
- `release-please.yml` and `release.yml`
- `deploy-backend.yml`
- `pages.yml`, cut down to the OpenAPI check and a Swagger-UI-only Pages site, so the API docs stay public at `https://<owner>.github.io/logistics-backend/api-docs/`

The frontend and Android workflows move out.

**Frontend:**
- **Unit tests:** Vitest, type-check and lint, as today.
- **Playwright end to end** needs a running API. The job checks out
  `logistics-backend` at the `backend_ref` input (default `master`), builds it
  with cargo (cached) and starts it next to MySQL, as today's job does in one
  repo. The GHCR image isn't used, because it is built for arm64 only and the
  runners are x86_64. A daily scheduled run catches API changes that break the
  dashboard even when the frontend didn't change.
- **Pages:** the dashboard at `https://<owner>.github.io/logistics-frontend/`,
  with `VITE_API_BASE_URL` from the repo variable `API_BASE_URL` (moved from
  the backend repo).

**Android:** today's `android.yml` (unit tests, lint, debug and unsigned
release builds), now running on every push since the whole repo is the app.

## Release tags, separately per repo

Each repo runs its own release-please, so its tags, changelog and GitHub
Releases only describe its own code.

| Repo | Release type | Version file(s) | Starts at | Release artifacts |
| --- | --- | --- | --- | --- |
| Backend | `rust` | `Cargo.toml` | continues its current version | GHCR image `logistics-api`, `openapi-X.Y.Z.json` |
| Frontend | `node` | `package.json` | the version it has at the split (shared with the backend until then) | `frontend-X.Y.Z.tar.gz` (static build) |
| Android | `simple` | `version.txt`, read by Gradle | `0.1.0` (today's `versionName`) | release notes only. The signed APK is still built locally (`build-release.sh`), because the signing key must never reach CI. |

- **Android `versionCode`:** it is derived from the version in Gradle as
  `major × 10000 + minor × 100 + patch`. It goes up with every release without
  hand edits (0.1.0 → 100, today it's 1), so Android always accepts the update.
- **Backend:** `extra-files` stops bumping `frontend/package.json`.
- **Old combined tags:** v0.1.0 to v0.14.x stay in the backend repo. The new
  repos start without tags. Their release-please configs set `bootstrap-sha`
  to the split commit, so the first release's changelog only covers work after
  the split. Their manifests start at the frontend's version at the split and
  at Android 0.1.0.

## Working across repos

- **Local layout:** sibling checkouts in one parent folder:
  `logistics-backend/`, `logistics-frontend/`, `logistics-android/`.
- **Frontend dev and demo servers:** `playwright.demo.config.ts` starts
  `cargo run` in `BACKEND_DIR` (default `../logistics-backend`). Vite's
  `API_PROXY_TARGET` is unchanged.
- **Android demos:** `android/scripts/lib.sh` starts the backend from
  `BACKEND_DIR` (default `../logistics-backend`), and the dashboard demos run in
  `FRONTEND_DIR` (default `../logistics-frontend`).
- **Shared contracts:** comments that point across repos ("mirrors
  `src/logistics/...`") become repo-qualified links. The OpenAPI spec published
  with each backend release is the shared contract.
- **`todo.org`:** stays in the backend repo, as the one task list for all
  three.

## Cutover order

Nothing on GitHub changes until the user says go. Until then everything
happens locally, so it can be redone from a newer `master`.

1. **Prepare locally (reversible):**
   - Extract both new repos with `git subtree split`.
   - Add each one's README, workflows and release config as commits on top.
   - Run their builds and tests locally.
   - Prepare the backend cleanup branch.
2. **Land in-flight work first**, or note how to port it:
   - Open PRs: the release PR #101 (v0.15.0) is merged or closed first, so
     the last combined release is clear.
   - Branches that touch `frontend/` or `android/`, including the main
     checkout's uncommitted `refactor/org-page-extract-sections` work, are
     merged before the split. If one isn't, its frontend part is moved over
     afterwards with `git format-patch --relative=frontend` and `git am` in
     the frontend repo.
3. **Go (one session):**
   - Re-extract from the latest `master` if it moved.
   - Create the two repos and push their history and tags.
   - Set Pages to GitHub Actions, the `API_BASE_URL` variable, and the
     `ANTHROPIC_API_KEY` secret (which the user adds; secret values can't be
     read back from GitHub).
   - Merge the backend cleanup PR.
   - Rename the repo to `logistics-backend`.
   - Watch all three repos' CI until green.
4. **Afterwards:**
   - Point the local ngrok deployment at the two checkouts.
   - Update the README badges.
   - Re-clone or move local worktrees.
   - Retire `docs/repo-split.md` to a short "as built" note.

## Risks

- **Pages URLs change:** `/logistics/` → `/logistics-frontend/` (dashboard)
  and `/logistics-backend/api-docs/` (Swagger). GitHub doesn't redirect Pages
  after a rename. Nothing in the repo links to the old Pages URL except the
  README.
- **Contract drift** between dashboard and API is caught later than in one
  repo: by the frontend's scheduled and per-push e2e runs against the
  backend's `master`, not by the backend PR itself.
- **The GHCR image name** stays `logistics-api`, so deploys don't change.
