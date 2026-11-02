# RAG across the logistics system (exploration and plan)

Status: **exploration** (2026-11-02). Where retrieval-augmented generation
(RAG) already is in the system, what limits it, where else it would earn its
keep, and a phased plan. Nothing here is built yet.

## Summary

The system already has one real RAG feature: the dashboard's "ask your data"
assistant. It runs on MySQL FULLTEXT search over a write-through chunk index
(`AiChunks`), plus a live OpsReport snapshot. The other AI features (dispatch
summary, report narration, daily digest, reorder suggestions, notification
copy) are prompts over structured data, and should stay that way.

The highest-value next steps are not new vector infrastructure:

1. **Make the existing assistant work in production.** The deployed backend
   has no `ANTHROPIC_API_KEY`, so every AI feature is off. Six modules also
   each hard-code `claude-haiku-4-5-20251001` with their own copy of the HTTP
   call.
2. **Fix retrieval failures that were measured** (below): partial
   registration numbers, plurals and synonyms find nothing today.
3. **Cited answers** that link to the dispatch, vehicle or document they came
   from, using Claude's `search_result` blocks with citations.
4. **Let the model look things up itself:** tool use over the queries the
   app already has (a dispatch by id, a vehicle by number, overdue
   dispatches), next to the FULLTEXT search.
5. **New surfaces where the data is unstructured** or the reader is not a
   dispatcher:
   - the driver's own assistant in the Android app;
   - uploaded compliance PDFs and an org's own rules or SOPs;
   - in-app help over the product docs.

Embeddings come last, and only if an evaluation shows the cheaper fixes
aren't enough.

## What exists today

| Piece | Where | How |
| --- | --- | --- |
| Chunk index | `src/logistics/ai/chunk.rs`, table `AiChunks` | One row per source `(org_id, kind, source_id)` with a FULLTEXT index on `text`. Write-through: every write to a source row upserts its chunk, best-effort; `reindex_org` backfills. |
| Indexed kinds | same | `notification`, `vehicle_document`, `vehicle_maintenance`, `dispatch_narrative` (rebuilt whole on every status change), `stock_snapshot`, `org_directory` (customers, godowns, vehicles with their driver) |
| Retrieval | `chunk::search_by_org` | `MATCH(text) AGAINST (q IN NATURAL LANGUAGE MODE)` filtered by `org_id` (the tenant boundary), top 8 |
| Generation | `ai/assistant.rs` | One Claude call: the OpsReport snapshot, then the retrieved facts, then the question, with "answer only from these facts". A canned answer when there's nothing to go on. |
| API and UI | `POST /api/orgs/{id}/assistant/ask`, `/reindex`, `/digest`; `AssistantWidget` in logistics-frontend | Single question and answer; sources listed as kind, id and excerpt |

### Size

Measured on the local dev database (MySQL **8.4**): the largest org has 44
chunks, and chunks average about 105 characters (the longest is 347). A busy
real org would have thousands, not millions. That matters for every option
below: comparing a question against every chunk of one org takes
microseconds, and no vector index or vector database is needed.

### What limits it

**Measured FULLTEXT misses** (dev database, natural-language mode):

| Query | Rows matched | Why |
| --- | --- | --- |
| `MH12DM9387` (full registration) | 1 | exact token |
| `9387` (what a dispatcher actually types) | **0** | the registration is a single token, so partial matches fail |
| `MH 12 DM 9387` (spaced, as on the plate) | **0** | the spaced parts don't match the single token |
| `cement` | 897 | |
| `cements` | **0** | no stemming |
| `delayed`, `late` | **0** | no synonyms, and the narrative text doesn't use those words |

Also: words shorter than 3 characters are dropped (`innodb_ft_min_token_size`
= 3), and the default stopwords list removes words such as "about".

**Other gaps:**

- **Off in production:** no `ANTHROPIC_API_KEY` on the deployed backend
  (`~/.config/logistics-ngrok/api.env`), and the `todo.org` item to set it is
  still open. The frontend repo's CI secret also still needs adding.
- **Old model in six places:** `claude-haiku-4-5-20251001` is hard-coded in
  `assistant.rs`, `status.rs`, `report.rs`, `digest.rs`, `reorder.rs` and
  `notification_copy.rs`, each with its own reqwest call. The current Haiku is
  `claude-haiku-5-5` at $0.10 / $0.50 per million input / output tokens,
  against $1 / $5 for Haiku 4.5. Moving needs a real migration: thinking is
  always on (control it with `effort`, default `medium`), sampling parameters
  and prefill are rejected, and the tokenizer is newer.
- **Single question, no memory:** a follow-up like "and the one before that?"
  is retrieved on its own words.
- **Sources aren't usable:** they're excerpts, not links, and the model
  doesn't say which sentence came from which fact.
- **Coverage stops at mid-2026 data:** vendors and their notes, vehicle
  hires and vendor payments, trips, invoices and payment status, stock
  transfers, proof-of-delivery details, and driver phone pairing / last
  report are not indexed.
- **No role scoping:** today's chunk kinds are fine for every role, but once
  billing is indexed, warehouse staff shouldn't get invoice answers.
- **No evaluation:** there's no set of known questions with expected sources,
  so a change to retrieval can't be measured.
- **Prompt injection:** retrieved text includes things users typed (notes,
  customer names, notification bodies). The prompt says "answer only from
  these facts" but doesn't mark that text as data.

## Where RAG fits, area by area

RAG helps where the answer depends on **text or history that a fixed query
can't anticipate**. It doesn't help where the answer is a number the app can
already compute; there, a prompt over the structured result (as the reports
and digest do) or a plain query is better.

| Area | Question it answers | Approach | Value | Effort |
| --- | --- | --- | --- | --- |
| Dashboard assistant, existing | "What happened with the Pune cement order last week?", "Which trucks have insurance expiring?" | Retrieval fixes, citations, tools, conversation | High: it exists but is off and misses common phrasings | Medium |
| Vendors and hired trucks | "Which vendor did we hire for Chakan last month and what do we still owe?" | New chunk kinds `vendor`, `vehicle_hire` (truck, driver, rate, payments, balance) | Medium | Low |
| Trips | "Which stops on yesterday's Nashik trip were late?" | `trip_narrative` chunk, rebuilt on change like dispatches | Medium | Low |
| Billing | "Has Shree Traders paid for the March deliveries?" | `invoice` chunk; **Admin and Dispatcher only** | Medium | Low, plus role scoping |
| Driver app (Android) | The driver asks "Where exactly do I unload?", "Who do I call at the site?" | Retrieval limited to **the driver's own assigned dispatches and trips**, behind the device token | High for drivers who can't call the office mid-drive | Medium |
| Compliance and other documents | "What's the insurance excess on MH12AB1234?", "Which permit covers Gujarat?" | PDF uploads, sent to Claude as `document` blocks with citations; small docs whole, large ones split into chunks | Medium | Medium: uploads are images-only today |
| An org's own rules and SOPs | "What's our loading rule for wet sand?", "Delivery hours for Hinjewadi sites?" | An org knowledge base: upload or paste text, chunked into `AiChunks` as kind `org_doc` | Medium–high for orgs with written rules | Medium |
| In-app help | "How do I dispatch on a hired truck?" | Retrieval over the product docs in `docs/` and the frontend README: one shared corpus, no org data | Medium: cuts onboarding questions | Low |
| Existing AI features, better context | Dispatch summary and delay alerts: "has this customer had late or returned deliveries before?" | Add the customer's 3–5 most relevant past dispatch narratives to the prompt | Medium | Low |
| Dispatch form suggestions | When creating a dispatch for a customer: the usual vehicle type, the last delivery notes, a past return reason | Retrieval only, no generation: show similar past dispatches | Medium | Low |
| Customer-facing status (WhatsApp/SMS reply) | "Where is my order?" | Retrieval limited to that customer's dispatches | High, but **risky** (outside users, abuse, wrong answers in public) | High; later |

**Not RAG, keep as is:**
- Fleet and report numbers: OpsReport is computed fresh and already injected.
- Reading expiry dates off an uploaded RC or insurance scan: that's
  extraction into `VehicleDocument`, a single call, not retrieval.
- Reorder suggestions: structured velocity maths.

## Architecture choices

### Retrieval

| Option | What | For | Against |
| --- | --- | --- | --- |
| **A. FULLTEXT, fixed** (recommended first) | Normalise identifiers when indexing and when querying (store `MH12DM9387` also as `MH 12 DM 9387`, `9387` and `DM9387`); a second FULLTEXT index `WITH PARSER ngram` on an `identifiers` column for partial matches; light query expansion (singular and plural, a small logistics synonym list: late/delayed/overdue, truck/vehicle/lorry); boolean mode for exact identifiers | No new vendor or service, small change, fixes every measured miss | Still lexical: misses paraphrases with no shared words |
| **B. Tools the model can call** (recommended second) | Give Claude tools backed by existing queries: `search_facts(query)` (option A), `get_dispatch(id)`, `find_vehicle(reg_fragment)`, `list_dispatches(status, customer, date range)`, `list_expiring_documents(days)`, `vendor_balance(vendor)`. The model picks; results come back as `search_result` blocks | Exact answers for structured questions, works with partial identifiers, follow-ups make sense | A few calls per question instead of one; needs a loop and tests per tool |
| C. Embeddings + hybrid (only if needed) | A second vendor's embedding API, since the Claude API has none. Vectors stored as BLOBs in a new `AiChunkVectors` table (MySQL 8.4 has no `VECTOR` type); cosine similarity in Rust over the org's chunks; results merged with FULLTEXT | Finds paraphrases and other-language queries | A second vendor, key and bill; re-embedding on every write-through; reverses a deliberate earlier decision |
| D. A vector database | Qdrant, pgvector and so on | — | Not justified at thousands of chunks per org |

### Generation

- **One shared client module** (`ai/client.rs`): one reqwest call, one model
  constant (`claude-haiku-5-5`), one place for the `effort` setting, timeouts,
  error mapping and usage logging. The six modules call it. Raw HTTP stays,
  because Rust has no official SDK.
- **Haiku 5.5, low effort,** for the assistant. Thinking is always on;
  `effort: "low"` keeps it short for Q&A. Measure against the evaluation
  before raising it.
- **Retrieved chunks as `search_result` blocks with citations enabled,**
  instead of a bulleted fact list. Claude then returns text with citations
  pointing at specific sources. The widget turns those into links: a
  dispatch narrative → the Dispatches page, a vehicle → `/vehicles/{reg}`, a
  document → the vehicle's documents. This also marks retrieved text as data,
  which helps against prompt injection.
  - One API detail to respect: from API version `2026-10-21`, `text` blocks
    with `citations` must not be sent back in a `user` message. Keep earlier
    answers as `assistant` turns, or send their text without citations.
- **Conversation:** keep the last few turns (questions, answers and the ids
  of their sources) in the widget, and send them with the next question.
  With tools (option B), the model can resolve "that dispatch" itself.
- **Prompt caching:** worth it only for a stable prefix of at least
  512 tokens on Haiku 5.5, such as the instructions, tool definitions and the
  OpsReport snapshot for an org within a short session. Retrieved chunks
  change per question, so they go after the cache breakpoint.

### Safety and correctness

- **Tenant isolation:** every retrieval path and tool takes `org_id` from the
  authenticated session, never from the model. A test per tool proves that
  another org's id returns nothing.
- **Role scoping:** chunk kinds get a minimum role (billing → Admin or
  Dispatcher). Retrieval filters by the caller's role; tools check the same
  role gates their REST routes do.
- **Driver scoping:** the driver assistant's corpus is the dispatches and
  trips assigned to that driver, resolved from the device token, plus the
  org's driver-facing rules. Never the org's whole index.
- **Writes stay out of the assistant:** it reads only. Actions such as
  "assign a truck" stay in the UI, or come later as explicit, confirmed tool
  calls.
- **Grounding:** keep the "answer only from the given facts" rule, the canned
  answer when nothing matches, and say plainly when sources disagree or are
  old (chunks carry `updated_at`).

### Evaluation (before changing retrieval)

- A fixture org with realistic data: about 30 questions, each with the source
  ids it should find. Include the measured misses, follow-ups,
  cross-entity questions ("vendor + truck + balance"), and questions that
  should be refused (another org, or another role).
- **Retrieval recall@8** runs in `cargo test`, with no Claude call, so CI
  catches regressions.
- **Answer quality** (grounded, cites the right source, refuses when it
  should) is a separate manual run against the live API, since it costs
  money. The `/claude-api build-eval` flow can set it up.

### Cost

At Haiku 5.5 prices, one question with about 2,000 input tokens (instructions,
snapshot, 8 short chunks) and a few hundred output tokens (including short
thinking at low effort) costs well under a tenth of a cent. A tool-using
question with a few round trips stays around a tenth of a cent. Usage should
be logged per org anyway (`usage` in each response), so that's checked
rather than assumed.

## Phased plan

Each phase ships per this repo's conventions: backend and logistics-frontend
PRs (backend first, because the frontend's e2e runs against backend
`master`), unit, route and Playwright tests, and a standalone headed demo.
Android work goes in logistics-android.

0. **Prerequisites**
   - Set `ANTHROPIC_API_KEY` on the deployed backend and as logistics-frontend's
     CI secret.
   - Add `ai/client.rs` and move all six modules to it.
   - Migrate to `claude-haiku-5-5`, with per-feature `effort`.
1. **Evaluation and retrieval fixes (option A)**
   - The fixture org and the question set; recall@8 in `cargo test`.
   - Identifier normalisation, an ngram index for identifiers, and query
     expansion. Done when every measured miss above is found.
2. **Cited answers**
   - `search_result` blocks with citations.
   - Sources become links in the widget.
   - Retrieved text marked as data.
3. **Coverage and roles**
   - New chunk kinds: `vendor`, `vehicle_hire`, `trip_narrative`,
     `invoice`, `stock_transfer`, `driver_phone`.
   - A minimum role per kind, filtered at retrieval.
   - `reindex_org` covers the new kinds.
4. **Tools and follow-ups (option B)**
   - The structured lookup tools (`get_dispatch`, `find_vehicle`, ...).
   - Conversation turns in the widget.
5. **Driver assistant (Android)**
   - `POST /api/driver/assistant/ask`, authenticated by the device token and
     scoped to the driver's assignments.
   - An "Ask" screen in the app.
   - The emulator demo checks that the answers come only from that driver's
     dispatches.
6. **Documents**
   - PDF uploads for compliance documents.
   - An org knowledge base (`org_doc` chunks).
   - In-app help over the product docs.
7. **Embeddings (conditional, option C):** only if phase 1's evaluation still
   shows paraphrase misses that matter.

## Questions for the user

- Phase 0 needs an Anthropic API key on the deployed server and in
  logistics-frontend's secrets. Should the AI features be enabled in the
  hosted deployment at all yet?
- Which audience comes first after the dashboard: drivers (phase 5) or
  document Q&A (phase 6)?
- Billing in the assistant: Admin and Dispatcher only, or Admin only?
- Is a second vendor for embeddings acceptable if phase 1 isn't enough, or
  should retrieval stay within MySQL and the Claude API?
