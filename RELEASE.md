# Release process

**No release has been published.** Nothing has been signed for distribution,
uploaded to a download host, or listed in a store.

## Release candidate (no approval needed)

1. `powershell -File tools\check.ps1`: every check CI runs (format, lint,
   all tests, screen tests, memory gate, screen request budgets, website
   build) plus a macOS compile check of the engine crates. Use it when GitHub
   Actions is unavailable too.
2. `powershell -File tools\release.ps1`: builds the Windows installer,
   installs it on this computer and smoke-tests the installed copy
   automatically (right-click entries, window appears and stays responsive on
   a disposable folder, local log records the start, closing behaves), then
   writes `target\release\bundle\manifest.json` (file, size, SHA-256,
   signed = false, verified = false).
3. Mac: `gh workflow run release.yml -R hullashoo6/tessera` builds the Windows
   installer and the universal macOS DMG on GitHub's runners as private
   artifacts (`gh run download <id> -R hullashoo6/tessera -D dist-local\ci`).
   Needs GitHub Actions minutes; there is no other way to build the DMG
   without a Mac.
4. Manual checks on each installer (TESTING.md): cleanup of disposable files
   and Put back, licence activate/check/deactivate with a test key, File
   Explorer's or Finder's right-click entry, uninstall. Record the results in
   STATUS.md. Only then run `tools\release-manifest.ps1 -Verified`.

## Microsoft Store (owner's free Partner Center account needed)

`tools\make-msix.ps1 -Name <Identity/Name> -Publisher "CN=..." -PublisherDisplayName "..."`
builds the MSIX package from the same release build (see docs/STORE.md).
Store copies leave updates to the Store. **Submission needs the owner's OK.**

## Needs the owner's approval

* Publishing anything: website download links (`website/src/data/releases.json`
  from a verified manifest, and `latest.json`), a Store submission, a public
  GitHub release.
* Paid signing: Windows Authenticode or Azure Trusted Signing; Apple Developer
  Program (USD 99/year) for notarisation. Unsigned builds are labelled as such,
  and customers are never told to turn off security protections; the Mac
  installer window explains the one-time "Open Anyway" step.
* Turning on live sales (`LIVE_MODE_APPROVED`, `pricing.onSale`).
