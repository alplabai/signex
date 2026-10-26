# Cached distributor pricing in the `.snxlib`

Status: implemented (issue #152, "Refresh All Pricing").

## What is stored

A component row already binds a manufacturer part (`primary_mpn`: manufacturer
and MPN) and distributor listings (`supply`: distributor name and SKU). What it
lacked was the last price the distributor reported. That is now kept per row
and per distributor as a `CachedPricing`:

| Field | Meaning |
|---|---|
| `sku` | The distributor part number priced: the listing SKU, or for a SKU-less listing priced by MPN the part number the distributor reported. |
| `currency` | ISO 4217 code of every price. |
| `stock` | Units in stock at fetch time, if reported. |
| `breaks` | `PriceBreak { qty, unit_price }`, ascending by quantity. |
| `fetched_at` | When the distributor was asked, UTC (RFC 3339). |

`PriceBreak` and `PricingSnapshot` (the type distributor clients return from
`refresh_pricing`) are the existing types. `PriceBreak::unit_price_usd` became
`unit_price` (the old name is accepted on read) and `PricingSnapshot` gained
`currency` and `stock`, both defaulted, so older payloads still deserialize.

## Where it lives in the file

The component TSV header is a fixed 21-column schema checked on every read, so
a new column would change every existing library. The cache is instead a
per-table sidecar, the same mechanism as `[tables.<name>.column_types]`:

```toml
[tables.resistors.pricing.0192a8c0-1111-7000-8000-000000000001.DigiKey."311-10.0KHRCT-ND"]
sku = "311-10.0KHRCT-ND"
currency = "USD"
stock = 50000
fetched_at = "2026-10-24T09:30:15.123456789Z"
breaks = [{ qty = 1, unit_price = 0.1 }, { qty = 100, unit_price = 0.05 }]
```

- The sections are written only when a table has cached prices. A library with
  none is byte-identical to one written before this existed, and re-saves
  byte-identical.
- The format token stays `snxlib/1`. The change is additive and optional; a
  build that predates it ignores the unknown sub-tables (the TOML layer does
  not reject unknown keys) and keeps working, at the cost of dropping the cache
  the next time it saves. Bumping the token would instead make older builds
  refuse the file, which is the wrong trade for a cache that can be re-fetched.
- Deleting a row deletes its cached prices. A price whose `sku` no longer
  matches the row's listing is ignored by the display, refused by `store_pricing`, and removed silently by every other save (see Records, pruning and damaged caches).

## Finite-number rule

Every price must be finite and not negative, quantities start at 1, and the
currency is a three-letter code. This is enforced when a snapshot is converted
to `CachedPricing`, when a table is written (`LibraryFileError::InvalidPricing`),
and when a file is read (TOML accepts `nan` and `inf` literals, so a hand-edited
file can carry them). The writer also re-reads its own output and refuses a
document that does not reproduce the cached prices.

## Refresh

`signex_library::refresh_library_pricing` reads the rows, asks each distributor
client for every listing that names it (a SKU prices that exact part; a listing
without one is priced by MPN and maker) (one distributor at a time, one row
at a time; the clients space their own requests), and writes every fetched
price back with `LibraryAdapter::store_pricing`: one write lock, one reload,
one save, one commit. Each price is re-checked under the lock; a row deleted, or
re-pointed at another SKU, while the fetch ran is reported and skipped.

An auth or rate-limit failure ends that distributor's pass and the remaining
rows carry the same reason. A failed row keeps its previous cached price.

## Which clients return prices

`DistributorAdapter::supports_pricing()` says whether a client's response
parser reads price breaks. It is true for the Mouser client (`PriceBreaks[]`
with `Quantity`, `Price`, `Currency`; stock from `AvailabilityInStock`) and the
DigiKey client (`ProductVariations[].StandardPricing[]`; the variation
numbered like the row's SKU, currency from the `X-DIGIKEY-Locale-Currency`
request header). LCSC and JLCPCB stay false: their response models carry no
price fields. `distributors::PRICING_SOURCES` mirrors the answers so the menu
can decide without building a client; a test keeps the two equal. Refresh All
Pricing is greyed while the distributor order (Distributor APIs settings)
lists no source that returns prices. It does not check credentials: with none
set, each group fails with an auth message. A walk
reports a client that does not (`<name> does not return prices yet`) instead of
asking it for every row.

A row's SKU selects the exact part: DigiKey `…/{sku}/productdetails`, Mouser
`/search/partnumber`. A SKU miss is a `NotFound`; it never falls back to an MPN
search (DigiKey prices only the packaging numbered like the SKU). A row with a
SKU is priced even when its MPN is empty. Mouser also reports a bad key or an
exhausted quota as HTTP 200 with `Errors[]` populated; a non-empty list maps to
Auth, RateLimited or Backend. Auth and RateLimited halt the walk for that
distributor; a Backend error fails only that row and the walk moves on.

Cancelling a walk (library close, app quit) discards what it fetched and logs
one summary line, not one per unvisited row. Closing a project does not cancel
it: the library stays mounted and may be shared with another project.

## BOM price columns

Implemented in #1023; see [In the BOM](#in-the-bom-1023) below.

## Records, pruning and damaged caches

- A record is keyed by row, distributor and part number, so a cut-tape and a
  reel listing of one distributor keep separate prices. `sku` is the
  distributor part number priced: the listing's SKU, or, for a listing with no
  SKU that was priced by exact MPN and maker, the part number the distributor
  reported.
- A client reports which part it priced; the cache refuses a price whose part
  differs from the listing's SKU. SKU lookups accept only an exact part-number
  hit and never fall back to a fuzzy MPN search.
- Every save of the library drops records whose row is gone or whose part
  number the row no longer lists.
- Storing a price replaces any record whose part number matches case- and
  whitespace-insensitively, so a retyped SKU never leaves the old price behind.
- On load, a damaged cache entry at any level (a row or distributor entry that
  is not a table, a bad field, empty breaks, negative price) is dropped with a
  warning naming it, because it can be fetched again. A
  non-finite number anywhere in a record still refuses the load, as for every other format (#924,
  #828).
- DigiKey claims prices only when it sends `X-DIGIKEY-Client-Id`, which a
  client built from `DigiKeyAuth` does.

## In the BOM (#1023)

The BOM preview and every BOM export price their lines from these caches,
without a network call and without reading a file: the app builds a
`signex_bom::PriceBook` from the rows and prices the open libraries already
hold in memory (`OpenLibrary::tables` / `pricing`).

- **Join.** A placed symbol links to its row by `(library_id, row_id)`, which
  placement records. A part placed without that link, or whose row has no
  cached price, is matched by MPN (the row's `primary_mpn`, against the
  symbol's `MPN`, `Manufacturer Part Number` or `Part Number` field,
  case-insensitive). Parts from different rows never group into one line.
- **Which price.** The first distributor in the row's listing order with a
  current price (as the library browser shows it). A line is priced at its
  fitted quantity: the largest break at or below it, or the first break when
  the quantity is under the minimum order.
- **Columns.** Unit Price, Ext. Price (unit price × fitted quantity),
  Currency, Stock, Distributor and Price Age. Unpriced lines leave them empty.
- **Totals.** With Ext. Price shown, the exports end with one total line per
  currency; prices are never converted between currencies. The preview's
  footer shows the same totals and how many lines have no cached price.
