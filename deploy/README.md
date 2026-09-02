# Deployment

Free-tier setup: the **frontend + Swagger UI** on GitHub Pages, the **Rust API
+ MySQL** on an Oracle Cloud "Always Free" VM, both driven from GitHub Actions.

```
 Browser ──HTTPS──▶ <owner>.github.io/<repo>/         (React dashboard, GitHub Pages)
    │                     /api-docs/                  (Swagger UI)
    │
    └───────HTTPS──▶ logi-api.duckdns.org  ──▶ Caddy ──▶ api:8080 ──▶ db:3306
                     (Oracle VM: docker compose — caddy + api + mysql + duckdns)
```

## Workflows

| Workflow | Trigger | Does |
| --- | --- | --- |
| `.github/workflows/pages.yml` | push to `main`/`master`, manual | Builds the Vite app, generates + validates the OpenAPI spec, deploys both to Pages — this is the live demo, always tracking `master` |
| `.github/workflows/release-please.yml` | push to `master` | Maintains a rolling release PR (version bump + `CHANGELOG.md`); merging it tags `vX.Y.Z` |
| `.github/workflows/release.yml` | `v*` tag, manual | Builds + pushes the GHCR image `:X.Y.Z`/`:X.Y`/`:latest`, publishes the frontend bundle + OpenAPI spec on a GitHub Release, optionally rolls the VM to that version |
| `.github/workflows/deploy-backend.yml` | manual | Break-glass: pin the VM to any existing GHCR image tag (redeploy / rollback) |

Full release process: [`docs/releasing.md`](../docs/releasing.md).

## One-time setup

### 1. GitHub Pages
Repo → **Settings → Pages → Source → GitHub Actions**.

### 2. Repo variable for the API URL
Repo → **Settings → Secrets and variables → Actions → Variables → New**:

| Name | Value |
| --- | --- |
| `API_BASE_URL` | `https://logi-api.duckdns.org/api` |

The frontend build fails fast if this is missing.

### 3. DuckDNS
Sign in at <https://www.duckdns.org>, create a subdomain (e.g. `logi-api`),
copy your token.

### 4. Oracle Cloud "Always Free" VM
- Create an **Ampere A1 (ARM)** instance, Ubuntu 22.04, with a public IP.
- Security list / NSG: allow inbound **TCP 80, 443** (and 22).
- On the VM also open the host firewall:
  ```bash
  sudo iptables -I INPUT -p tcp --dport 80 -j ACCEPT
  sudo iptables -I INPUT -p tcp --dport 443 -j ACCEPT
  sudo netfilter-persistent save
  ```
- Install Docker + compose plugin:
  ```bash
  curl -fsSL https://get.docker.com | sh
  sudo usermod -aG docker "$USER" && newgrp docker
  ```
- Point DuckDNS at the VM's public IP once (the `duckdns` container keeps it
  updated afterwards).

### 5. Deploy config on the VM
```bash
mkdir ~/logistics-deploy && cd ~/logistics-deploy
# copy deploy/.env.example from the repo to .env and fill every value
```
Notable values in `.env`:
- `JWT_SECRET` — **required**; a 32+ character random string
  (`openssl rand -base64 48`). The API refuses to start without it.
- `API_IMAGE` — pin to a released version, e.g.
  `ghcr.io/<owner>/logistics-api:0.1.0`.
- `ANTHROPIC_API_KEY` — optional (AI dispatch summaries).
- `ANTHROPIC_WORKSPACE_ID` — only if the key above is identity-linked rather
  than workspace-scoped; otherwise the summary call fails with
  `workspace_id_required`.

### 6. GitHub Actions secrets
Repo → **Settings → Secrets and variables → Actions → Secrets**:

| Name | Value |
| --- | --- |
| `SSH_HOST` | VM public IP |
| `SSH_USER` | `ubuntu` (or `opc`) |
| `SSH_KEY` | private key (full PEM) whose public half is in `~/.ssh/authorized_keys` on the VM |
| `SSH_PORT` | optional, default `22` |

### 7. GHCR image visibility
After the first release publishes the image, either:
- make the package public: GitHub → your profile → Packages → `logistics-api`
  → Package settings → Change visibility → Public; **or**
- keep it private and run `docker login ghcr.io -u <user> -p <read:packages PAT>`
  on the VM.

### 8. Release-related repo settings (optional)
- Variable `DEPLOY_ON_RELEASE=true` — `release.yml` also rolls the VM to the new
  version. Requires the `SSH_*` secrets from step 6.
- Secret `RELEASE_PLEASE_TOKEN` — a PAT so a release-please tag auto-triggers
  `release.yml`. See [`docs/releasing.md`](../docs/releasing.md).

## First run

1. Merge this branch. `pages.yml` publishes the site (live demo, tracks `master`).
2. Cut the first release: `git tag v0.1.0 && git push origin v0.1.0`. `release.yml`
   builds the image and (if `DEPLOY_ON_RELEASE=true`) brings the VM stack up;
   otherwise run **Deploy Backend (manual)** with `image_tag = 0.1.0`.
3. Visit `https://logi-api.duckdns.org/api/health` → `{"status":"ok"}` (Caddy
   may take ~30s to obtain the first certificate).
4. Open the Pages URL, register an org, log in.

## Backups

MySQL data lives in the `db-data` Docker volume on the VM. A minimal daily dump:
```bash
mkdir -p ~/backups
( crontab -l 2>/dev/null; echo '15 2 * * * cd ~/logistics-deploy && docker compose --env-file .env exec -T db sh -c "mysqldump -uroot -p\$MYSQL_ROOT_PASSWORD \$MYSQL_DATABASE" | gzip > ~/backups/logistics-$(date +\%F).sql.gz && find ~/backups -name "*.sql.gz" -mtime +14 -delete' ) | crontab -
```
Restore: `gunzip < backup.sql.gz | docker compose --env-file .env exec -T db sh -c 'mysql -uroot -p$MYSQL_ROOT_PASSWORD $MYSQL_DATABASE'`.

## Architecture note

Oracle's free Ampere VMs are ARM64, so `release.yml` builds a single-arch
`linux/arm64` image on GitHub's native `ubuntu-24.04-arm` runner (free for
public repos). Cross-building this crate for arm64 under QEMU emulation crashes
rustc, so emulation is avoided entirely.

If you deploy to an x86 box instead (e.g. the `VM.Standard.E2.1.Micro` free
tier — 1 GB RAM, give MySQL a tight `innodb_buffer_pool_size`), switch
`release.yml`'s build job back to `ubuntu-24.04` and the platform to
`linux/amd64`. For both, use a matrix over the two native runners and merge
with `docker buildx imagetools create` — never a single QEMU multi-arch build.

## Local production-style run

```bash
cd deploy
cp .env.example .env      # fill in
docker compose --env-file .env up --build
```
