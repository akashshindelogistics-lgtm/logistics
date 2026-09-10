# CI on every branch push

## Problem

The three test workflows only triggered on `push` to `main` / `master` plus
`pull_request` against those branches:

```yaml
on:
  push:
    branches: [main, master]
  pull_request:
    branches: [main, master]
```

So when you pushed a feature branch (`todo-*`, `worktree-*`, …) nothing ran
until you opened a pull request. You could push many commits with no signal,
and a green local run was the only evidence the branch was healthy. The task in
`todo.org` was: *"All CI workflows should run on push to branch."*

## Change

`push` now matches **every branch** for the CI workflows:

```yaml
on:
  push:
    branches: ['**']
  pull_request:
  workflow_dispatch:
```

| Workflow | Before | After |
| --- | --- | --- |
| `frontend-unit-tests.yml` | push to `main`/`master` | push to any branch |
| `frontend-integration.yml` | push to `main`/`master` | push to any branch |
| `periodic-tests.yml` | push to `main`/`master` (+ 2h schedule) | push to any branch (+ 2h schedule, unchanged) |
| `pages.yml` | push to `main`/`master` → validate + build + deploy | push to any branch → validate + build; **deploy only on `main`/`master`** |

### Avoiding duplicate runs

With `push` on all branches, a same-repo branch that also has an open PR would
otherwise run each workflow twice (once for `push`, once for `pull_request`).
Two guards prevent that:

1. Every CI job carries

   ```yaml
   if: >-
     github.event_name != 'pull_request' ||
     github.event.pull_request.head.repo.full_name != github.repository
   ```

   so the `pull_request` event only does anything for **forks** — which cannot
   trigger `push` on this repo and would otherwise get no CI at all.

2. A `concurrency` group per `(workflow, event, branch)` with
   `cancel-in-progress: true`, so pushing again to the same branch cancels the
   superseded run. The event is part of the key so a `schedule` run of
   `periodic-tests.yml` is never cancelled by a branch push.

### `pages.yml`

The workflow both validates the OpenAPI spec (a genuine CI check — catches
route/spec drift) and publishes the live GitHub Pages site. Only the trigger
widened; the `deploy` job gained

```yaml
if: ${{ github.ref == 'refs/heads/main' || github.ref == 'refs/heads/master' }}
```

so a feature branch runs `validate-swagger` + `build-frontend` as checks and
stops there — it never republishes the public site.

## Not changed

`release-please.yml` (push to `master`), `release.yml` (`v*` tags) and
`deploy-backend.yml` (`workflow_dispatch`) are release / deploy automation, not
CI. Running them per branch would cut releases or touch the production VM from
unmerged code, so their triggers are left alone.

## Cost

Each branch push now runs the Rust build + Cargo suite, the Playwright suite
against a MySQL service container, the Vitest suite, and an OpenAPI-spec build.
The Cargo/npm caches are shared by key, and `cancel-in-progress` keeps only the
newest run per branch alive, so the steady-state cost is roughly one full CI
pass per push instead of per PR.
