# Cached distributor pricing in the `.snxlib`

Status: implemented (issue #152, "Refresh All Pricing").

## What is stored

A component row already binds a manufacturer part (`primary_mpn`: manufacturer
and MPN) and distributor listings (`supply`: distributor name and SKU). What it
lacked was the last price the distributor reported. That is now kept per row
and per distributor as a `CachedPricing`:

| Field | Meaning |
|---|---|
| `sku` | The listing SKU the price was fetched for. |
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
[tables.resistors.pricing.0192a8c0-1111-7000-8000-000000000001.DigiKey]
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
  matches the row's listing is ignored by the display and refused at write time.

## Finite-number rule

Every price must be finite and not negative, quantities start at 1, and the
currency is a three-letter code. This is enforced when a snapshot is converted
to `CachedPricing`, when a table is written (`LibraryFileError::InvalidPricing`),
and when a file is read (TOML accepts `nan` and `inf` literals, so a hand-edited
file can carry them). The writer also re-reads its own output and refuses a
document that does not reproduce the cached prices.

## Refresh

`signex_library::refresh_library_pricing` reads the rows, asks each distributor
client for the rows that list a SKU with it (one distributor at a time, one row
at a time; the clients space their own requests), and writes every fetched
price back with `LibraryAdapter::store_pricing`: one write lock, one reload,
one save, one commit. Each price is re-checked under the lock; a row deleted, or
re-pointed at another SKU, while the fetch ran is reported and skipped.

An auth or rate-limit failure ends that distributor's pass and the remaining
rows carry the same reason. A failed row keeps its previous cached price.
