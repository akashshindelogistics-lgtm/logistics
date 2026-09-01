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
| `.github/workflows/pages.yml` | push to `main`/`master`, manual | Builds the Vite app, generates + validates the OpenAPI spec, deploys both to Pages |
| `.github/workflows/deploy-backend.yml` | push touching `src/**`, `Dockerfile`, `deploy/**`, manual | Builds the API image → GHCR, SSHes to the VM, `docker compose pull && up -d` |

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
The image is built ARM-compatible automatically only if you build on ARM — see
"ARM note" below.

### 6. GitHub Actions secrets
Repo → **Settings → Secrets and variables → Actions → Secrets**:

| Name | Value |
| --- | --- |
| `SSH_HOST` | VM public IP |
| `SSH_USER` | `ubuntu` (or `opc`) |
| `SSH_KEY` | private key (full PEM) whose public half is in `~/.ssh/authorized_keys` on the VM |
| `SSH_PORT` | optional, default `22` |

### 7. GHCR image visibility
After the first backend deploy, either:
- make the package public: GitHub → your profile → Packages → `logistics-api`
  → Package settings → Change visibility → Public; **or**
- keep it private and run `docker login ghcr.io -u <user> -p <read:packages PAT>`
  on the VM.

## First run

1. Merge this branch. `pages.yml` publishes the site.
2. Run **deploy-backend** (push or "Run workflow"). It builds the image and
   brings the stack up.
3. Visit `https://logi-api.duckdns.org/api/health` → `{"status":"ok"}` (Caddy
   may take ~30s to obtain the first certificate).
4. Open the Pages URL, register an org, log in.

## ARM note

Oracle's free VMs are ARM64. `docker/build-push-action` in `deploy-backend.yml`
builds `linux/amd64` by default. Either:
- add `platforms: linux/arm64` to the build step (slower — uses QEMU emulation
  in CI), **or**
- add `linux/amd64,linux/arm64` for a multi-arch image, **or**
- pick an x86 "Always Free" `VM.Standard.E2.1.Micro` instead (only 1 GB RAM —
  give MySQL a tight buffer pool).

## Local production-style run

```bash
cd deploy
cp .env.example .env      # fill in
docker compose --env-file .env up --build
```
