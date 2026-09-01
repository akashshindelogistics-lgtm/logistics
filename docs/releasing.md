# Releasing

Versions follow [SemVer](https://semver.org/). The backend (`Cargo.toml`) and
frontend (`frontend/package.json`) are released together under one version.

## How it fits together

```
 conventional commits on master
        │
        ▼
 release-please.yml ──► opens/updates a "release PR"
        │                (bumps Cargo.toml + package.json, writes CHANGELOG.md)
        │  merge the PR
        ▼
 tag vX.Y.Z  +  GitHub Release created by release-please
        │
        ▼
 release.yml ──► GHCR image  logistics-api:X.Y.Z / :X.Y / :latest   (arm64)
             ├─► frontend-X.Y.Z.tar.gz   (attached to the Release)
             ├─► openapi-X.Y.Z.json      (attached to the Release)
             └─► (optional) rollout to the Oracle VM, pinning API_IMAGE=:X.Y.Z
```

The live GitHub Pages demo keeps tracking `master` via `pages.yml` — releases do
not change it. A release is the reproducible checkpoint: the tagged image, the
static bundle, the spec, and the VM running that exact version.

## Prerequisites (one-time)

| What | Where |
| --- | --- |
| `API_BASE_URL` repo **variable** | e.g. `https://logi-api.duckdns.org/api` — the frontend build needs it |
| `RELEASE_PLEASE_TOKEN` repo **secret** *(optional)* | fine-grained PAT (contents + pull-requests: write). Without it, tags release-please pushes won't auto-trigger `release.yml` — run it via **Actions → Release → Run workflow** instead |
| `DEPLOY_ON_RELEASE` repo **variable** = `true` *(optional)* | makes `release.yml` also roll the new version out to the VM |
| `SSH_HOST` / `SSH_USER` / `SSH_KEY` secrets | required only if `DEPLOY_ON_RELEASE` is on (see `deploy/README.md`) |
| VM `~/logistics-deploy/.env` | `JWT_SECRET` set (32+ random chars), `API_IMAGE` pinned to a version |

## Cutting v0.1.0 (the first release)

release-please starts numbering from the last released tag; since there is none
yet, tag the first release by hand:

```bash
git checkout master && git pull
git tag v0.1.0
git push origin v0.1.0
```

That fires `release.yml` directly. From v0.1.1 on, use the release-please flow
below.

## Cutting v0.1.1+ (normal flow)

1. Land your changes on `master` with conventional-commit messages
   (`feat:`, `fix:`, `feat!:`/`BREAKING CHANGE:` for a major bump).
2. `release-please.yml` opens or updates a **release PR**. Review the proposed
   version and `CHANGELOG.md`.
3. Merge the release PR. release-please tags `vX.Y.Z` and creates the Release.
4. `release.yml` runs (automatically with `RELEASE_PLEASE_TOKEN`, otherwise
   trigger it manually for the new tag) and publishes the image + artifacts.
5. Deploy: if `DEPLOY_ON_RELEASE` isn't on, run **Actions → Deploy Backend
   (manual) → Run workflow** with `image_tag = X.Y.Z`, or on the VM edit
   `.env`'s `API_IMAGE` and `docker compose --env-file .env pull && up -d`.

## Rollback

**Actions → Deploy Backend (manual) → Run workflow** with `image_tag` set to a
previous version (e.g. `0.1.0`). Old image tags stay in GHCR.
