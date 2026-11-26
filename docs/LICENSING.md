# Licensing and payments

Owner: Beyond Enterprises (Dodo Payments account). Status: **live purchases
disabled**. No real Dodo test-mode licence has been checked yet; all
licensing tests so far use a simulated server.

## Products

| | Test mode | Live mode |
|---|---|---|
| Server | `https://test.dodopayments.com` | `https://live.dodopayments.com` |
| Pro product id | *pending (owner preparing)* | `pdt_0Nsmg4vQiRrOCZ4yTeDEk` |
| Licence entitlement | *pending* | `ent_0Nsmfr7CY18AXemTSmNxC` (dashboard reference) |
| Price | — | USD 9.78, one-time, tax included |
| Licence terms | — | 1 device, no expiry |

Configured in `crates/licensing/src/lib.rs` (`TEST`, `LIVE`,
`LIVE_MODE_APPROVED`). Product ids are public identifiers, not secrets.

## How the app checks a licence

The app uses Dodo's public endpoints, which need **no API key**:
`POST /licenses/activate`, `/licenses/validate`, `/licenses/deactivate`.

* A key grants Pro only if Dodo reports it belongs to the configured Pro
  product for the build's mode. Keys for other products are refused and
  their activation is released so they do not use up a seat.
* Test and live are fully separate: an activation made in one mode is
  ignored by the other and is never sent to the other server.
* While a mode has no product id, activation is refused.
* The activation is stored in Windows Credential Manager / macOS Keychain.
  Pro stays active offline for 30 days after the last successful check.
* Sent to Dodo: the licence key and a generic label such as
  "Tessera on Windows". Never file names, paths or scan results.

## Secrets

None are needed by the desktop app or the website. **Never** put a Dodo API
key or webhook secret in this repository, the app, or the frontend.

v1 needs no server: customers buy through Dodo's hosted checkout (a static
payment link built from the public product id) and receive the key by
email. A server is only needed if we later add checkout sessions created
on the fly or webhooks. That server would read, from its hosting
provider's secret settings (never from git):

| Variable | Purpose |
|---|---|
| `DODO_PAYMENTS_API_KEY` | Server-side API calls (creating checkout sessions) |
| `DODO_PAYMENTS_WEBHOOK_KEY` | Verifying webhook signatures |
| `DODO_PAYMENTS_ENVIRONMENT` | `test_mode` or `live_mode` |

## Gates before live purchases

1. Owner provides the **test** product id and a real test-mode licence key.
2. Set `TEST.pro_product_id`; activate, validate and deactivate the real test
   key from an installed build. Record evidence in STATUS.md.
3. Signed installers exist and pass installed-build testing.
4. Owner confirms live checkout verification is complete.
5. Owner approves; then set `LIVE_MODE_APPROVED = true`, release, and enable
   the website's purchase link (`website/src/data/site.ts`, `onSale`).
