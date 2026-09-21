# Stock categories: inventory items with different types

Tracked by the `todo.org` item *"Improve inventory management: let an org
add stock with different types/categories, and carry that categorization
through the transport (dispatch/transfer) side."*

## Model

- `Stock` (`src/logistics/stock/stock.rs`) gains `category: String`,
  defaulting to `"General"` (`default_category()`) wherever it isn't
  explicitly supplied — every caller/payload that predates this feature
  keeps working unchanged.
- **Free text, org-defined** — the same convention `description` already
  uses, not a fixed enum. An org can call its categories whatever makes
  sense to it ("Cement", "Electronics", "Perishable"...).
- **A pure attribute layered on top of `description`, not part of a stock
  item's identity.** `description` stays the sole uniqueness key per godown,
  exactly as before this feature — a convention, not a DB constraint. Two
  rows in the same godown can't disagree on category because they can't
  coexist under the same description at all; nothing stops two *different*
  godowns from holding the same description under different categories,
  though (see the resolution policies below).
- Set it with `Stock::new(...).with_category(...)`, mirroring
  `with_reorder_threshold`'s builder pattern exactly.

### Snapshotted onto a dispatch's line items

`DispatchLineItem` (`src/logistics/dispatch/dispatch.rs`) gains `category:
String`, copied from the matched stock item at dispatch time — the same way
`volume_in_size` already is. This is what actually answers "the transport
system carries stock of different types": a dispatch or trip's stops show
what *kind* of goods are on the truck, not just an item name, and that
categorization is fixed to the shipment's manifest even if the godown's
stock record is later re-categorized.

**Resolution policy — first matched godown wins.** `Organization::plan_stock_draw`
draws a requested quantity from however many godowns hold it, largest
holding first. Nothing enforces that two godowns hold the same description
under the same category, so if they disagree, the line item's category is
resolved from whichever godown is matched first (in `Godown::list_by_org`'s
order). This is stated explicitly so it's not a surprise — it mirrors the
existing `volume_in_size` resolution in the same function.

### Carried across a transfer

`StockTransfer` (`src/logistics/godown/transfer.rs`) gains `category:
String`. `StockTransfer::execute` mirrors its existing `effective_volume`
handling exactly:

**Resolution policy — destination wins on merge.** If the destination
godown already holds the item, its existing category is kept (the merged
row is not re-categorized to match the source). If the destination doesn't
hold it yet, a new row is created there using the source's category. The
audit row (`StockTransfers`) always records the *source's* category — what
actually moved — regardless of which category ends up on the merged
destination row.

### Godown inventory reporting

`GodownInventory` (`src/logistics/reports/mod.rs`) gains `category_breakdown:
Vec<CategoryUnits>` — one `{ category, units }` entry per distinct category
held in that godown, largest-units-first, ties broken alphabetically. This
answers the natural inventory-management question this feature exists for:
"which categories does this warehouse hold, and how much of each."

### Assistant narration

`stock_snapshot_text` (`src/logistics/ai/chunk.rs`) includes the category in
the narrative chunk indexed for the org-scoped assistant, e.g. `"Godown
'Chennai Central' (12 Anna Salai) holds 40 units of Cement (category:
Building Materials)."`

## API

- `POST /api/godowns/{gid}/stock` / `PUT /api/godowns/{gid}/stock`:
  `CreateStockPayload` / `UpdateStockPayload` both gain an optional
  `category` field (defaults server-side to `"General"` when omitted).
- `POST /api/godowns/{gid}/transfer`: unchanged request shape — category
  resolves server-side per the policy above and comes back on the
  `StockTransfer` response.
- `POST /api/dispatches` (dispatch stock / plan a trip): unchanged request
  shape — category resolves server-side and comes back on each
  `DispatchLineItem` in the response.
- `GET /api/orgs/{id}/reports`: `OpsReport.godown_inventory[]` entries now
  include `category_breakdown`.

## Frontend

- The org detail page's godown stock table gains a **Category** column
  (purple badge, matching the existing volume badge convention), and the
  inline "Add Stock" form gains an optional **Category** text field
  (blank defaults to `"General"`).
- The Dispatches page's line-item list shows each item's category next to
  its description. The Trips page's per-stop summary does the same.

## Known limitations

- No enforcement that all godowns agree on a description's category — see
  the two resolution policies above. An org that wants strict consistency
  has to maintain it by convention, the same way it already has to for
  `description` casing/spelling.
- No category-management UI (renaming a category across every stock row at
  once, for example) — categories are edited one stock row at a time, like
  every other stock field.
- The AI "add stock" action (on branches where it exists) is not
  category-aware yet; stock added that way gets the `"General"` default. A
  natural, separate follow-up.

## Tests

- Rust unit (`stock.rs`): category defaults to `"General"`; round-trips
  through `add_to_godown` → `list_by_godown`; `.with_category(...)`
  overrides it; `update_in_godown` changes and persists it.
- Rust unit (`orgs.rs`): a dispatched line item snapshots the matched
  stock's category; when two godowns disagree, the first-matched godown's
  category wins.
- Rust unit (`dispatch.rs`): category round-trips through `save()` →
  `get_by_id()`; a stock item returned via `credit_line_items_back` into a
  godown that didn't already hold it keeps the line item's category.
- Rust unit (`transfer.rs`): a transfer into a godown with no existing row
  carries the source's category; a transfer that merges into an existing
  row keeps the destination's category, while the audit row records the
  source's.
- Rust unit (`reports/mod.rs`): `godown_inventory`'s `category_breakdown`
  sums correctly across categories and sorts largest-first.
- Rust unit (`chunk.rs`): `stock_snapshot_text` includes the category.
- Rust route (`routes.rs`): add/update round-trip category through the API
  (including the omitted-field default); transfer carries it to the
  destination; dispatch returns it on the line item.
- Frontend unit (`godowns.test.ts`): `addGodownStock` includes category in
  the POST body, defaulting to `"General"`.
  (`OrganizationDetail.test.tsx`): the add-stock form fills and forwards
  category; the stock table renders a Category column.
  (`Dispatches.test.tsx`): a line item's category renders.
- Playwright (`organization.spec.ts`): fills the Category field via the UI
  and confirms it renders in the stock table row.
