/// <reference types="vite/client" />

interface ImportMetaEnv {
  /**
   * Base URL of the deployed API, including the `/api` prefix
   * (e.g. `https://logi-api.duckdns.org/api`). Unset in dev, where the
   * Vite proxy forwards the relative `/api` path to the local backend.
   */
  readonly VITE_API_BASE_URL?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
