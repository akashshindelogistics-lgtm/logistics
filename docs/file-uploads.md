# File uploads

Real file storage for uploaded images — today, proof-of-delivery photos and
signatures. Until now `ProofOfDelivery.signature_or_photo_url` was a
free-form `TEXT` column (a URL or `data:` URI); nothing in this repo stored
or served the actual image bytes, and the frontend's delivery-confirmation
form was a plain text input for pasting one in.

## Storage choice: local disk, not a cloud vendor

Uploaded files are written to disk under a configurable directory
(`UPLOAD_DIR`, default `./uploads`), not an S3-compatible bucket or other
object-storage vendor. This repo has one paid external dependency today
(`ANTHROPIC_API_KEY`) and deliberately avoided a second one for AI
embeddings (see the "ask your data" assistant's FULLTEXT-over-embeddings
design choice in `todo.org`); the same reasoning applies here — local disk
needs zero new credentials and works immediately in dev and a
single-instance deployment.

Revisit if/when the "finish up manual deploy steps" `todo.org` item lands
on a platform with an ephemeral filesystem (most container platforms wipe
local disk on redeploy) — swapping to an S3-compatible bucket later is a
contained change (one new module behind the same store/serve interface),
not a rearchitecture.

## Shape

`UploadedFile` (`src/logistics/upload/upload.rs`):

- `UploadedFiles` table: `id, org_id, content_type, byte_size, storage_path,
  uploaded_at`. `org_id` is the tenant-isolation boundary — the same role it
  plays in every other `list_by_org`-style table — so a file can be served
  back scoped to the org that uploaded it, rather than trusting a bare
  filesystem path from the client.
- `UploadedFile::store(org_id, content_type, bytes)` validates
  `content_type` against `ALLOWED_CONTENT_TYPES` (`image/jpeg`, `image/png`,
  `image/webp` — this pipeline exists for proof-of-delivery photos and
  signatures, not arbitrary file upload) and `bytes` against
  `MAX_UPLOAD_BYTES` (5 MiB), writes the bytes to
  `{UPLOAD_DIR}/{uuid}.{ext}`, and inserts the row.
- No schema change on `ProofOfDelivery`/`DispatchOrder` at all —
  `signature_or_photo_url` keeps being "a URL"; it's just now one this
  backend serves (`/api/uploads/{id}`) instead of an arbitrary external one.

## Routes

| Method & path | |
| --- | --- |
| `POST /api/orgs/{id}/uploads` | Admin \| Dispatcher. `multipart/form-data` with one `file` field. `400` on no file / unsupported content type / over the size limit. Returns `{ id, url }` |
| `GET /api/uploads/{id}` | Any authenticated org member. Streams the stored bytes back with the stored content type. `403` if the file belongs to a different org, `404` if unknown |

`POST` is gated to the same roles as `PUT /api/dispatches/{id}/status`
(`DISPATCH_ROLES` — Admin, Dispatcher), since uploading a POD photo happens
as part of that same delivery-confirmation flow.

## Frontend

The delivery-confirmation form's "Signature / Photo URL" text input became a
real `<input type="file" accept="image/jpeg,image/png,image/webp">`
(`src/pages/Dispatches.tsx`). Selecting a file uploads it immediately
(`uploadFile` in `src/api/uploads.ts`); "Confirm Delivery" stays disabled
until the upload resolves, and an inline error shows if it's rejected
(wrong type, too large, or a network failure). The existing "View
signature/photo" link keeps working unchanged — it's still just an `<a
href>`, now pointing at `/api/uploads/{id}`.

### Cross-origin deployments

The backend has no idea what its own public origin is (there's no
`PUBLIC_BASE_URL` config), so `POST /api/orgs/{id}/uploads` returns a
backend-relative path (`/api/uploads/{id}`), not an absolute URL. The
frontend resolves it to a usable link itself, the same way it already knows
where its own API lives — `uploadedFileHref()` in `src/api/uploads.ts`:

- No `VITE_API_BASE_URL` set (dev via the Vite proxy, or a same-origin
  production deployment): the path is used as-is, resolving against the
  frontend's own origin.
- `VITE_API_BASE_URL` set to a separate domain (the GitHub Pages frontend +
  DuckDNS API split described in the main README's Deployment section):
  the link is prefixed with that domain's origin, so it points at the API
  host that actually stores the file rather than 404ing against the
  frontend's own origin.

`uploadFile()` also has to explicitly unset the shared API client's default
`Content-Type: application/json` header for this one call
(`headers: { 'Content-Type': undefined }`) — otherwise axios would send the
`FormData` body as JSON instead of letting the browser compute the correct
`multipart/form-data; boundary=...` header itself.

## Tests

- Rust unit (`upload.rs`, no database mocking needed beyond the usual
  `TestDb`): rejects an unsupported content type / empty bytes / oversized
  bytes; a successful store writes real bytes to disk and round-trips
  through `get_by_id`; `get_by_id` returns `None` for an unknown id.
- Rust routes (`routes.rs`): a real upload is stored and served back byte-
  for-byte with the right content type; `401` with no token; `403` for a
  different org's token (both on `POST` and on `GET`); `403` for a
  non-dispatch role; `400` for an unsupported content type or a file over
  the size limit; `404` on `GET` for an unknown id.
- Frontend unit: `uploadFile` posts a `FormData` body with the
  `Content-Type` header unset; `uploadedFileHref` for the unset / same-domain
  / invalid-URL cases; the Dispatches page uploads a photo and only enables
  "Confirm Delivery" once it resolves, and shows an error (keeping the
  button disabled) when the upload fails.
- Playwright (`dispatches.spec.ts`, and `e2e-full-flow.spec.ts`'s existing
  delivery step): upload a real small in-memory PNG through the delivery-
  confirmation form, confirm delivery, then follow the "View
  signature/photo" link and fetch it directly to confirm it resolves to the
  uploaded bytes with the right content type. `npm run test:e2e:demo:uploads`
  is the standalone headed walkthrough (`file-uploads.demo.ts`).
