# Test org: E2E Test Logistics Co

A rehearsal org, loaded with `onboard_org` before Mauli Brickvision, so the
script and the onboarding steps are exercised on data we control. The
manifest is [`test-org-e2e-manifest.json`](test-org-e2e-manifest.json). It
holds no real business data, so it lives in git, not `onboarding-data/`.

It follows the tabs of the intake sheet
([`mauli-brickvision-intake-template.xlsx`](mauli-brickvision-intake-template.xlsx)):
Company, Stock Points, Products, Fleet (all five compliance papers plus a
service schedule), Drivers, Customers and Users. The records come from the
Playwright suite, minus the random `uid()` suffixes:

| Section | Records | From |
| --- | --- | --- |
| Godowns | Central Warehouse, Overflow Warehouse (max 10,000 L) | `e2e-full-flow.spec.ts`, `stock-categories.demo.ts` |
| Stock | Cement Bags, Steel Rods, Cable Reels, Cement, Office Chairs, Fittings, AAC Block 600x200x150 (from the size, 18 L per piece); Cement Bags and Cement in Overflow | full flow, categories, file-uploads, reports demos |
| Drivers | Ramesh Kulkarni, Trip Driver, Report Driver, and Inactive Driver (inactive on purpose) | full flow, trips |
| Vehicles | MH12DM0001 (60,000 L, all five documents, insurance expiring 2026-10-30 so the compliance alert shows), MH12TD0002 (16 t / 24 m³, so 21,333 L), MH12RP0003 (20,000 L) | full flow, trips, reports |
| Customers | Sunrise Traders, Moonlight Depot, Reachable Retail, Market Road Buyer, Upload Demo Customer, Trip Stop 1-3 | full flow, notifications, categories, file-uploads, trips |
| Users | Dana Dispatcher, Wes Warehouse, Ada Admin (`@e2e-test.example.com`) | roles demo |

Units are litres throughout, the same rule Mauli follows (see
[`../org-onboarding-script.md`](../org-onboarding-script.md)).

## Load it

```bash
cargo run --bin onboard_org -- --manifest docs/onboarding/test-org-e2e-manifest.json --validate-only
export ONBOARD_ORG_PASSWORD='Test@12345'   # owner password, same as the e2e helpers
cargo run --bin onboard_org -- --manifest docs/onboarding/test-org-e2e-manifest.json \
    --api-url http://127.0.0.1:8080 --credentials-out onboarding-data/test-org-e2e-logins.tsv
```

Then log in as the Dispatcher from the credentials file and send one
dispatch through to Delivered. The org has enough stock and free trucks for
the full-flow quantities (30 Cement Bags + 15 Steel Rods, a 10 + 8 trip).
