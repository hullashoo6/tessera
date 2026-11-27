# Microsoft Store (free Windows signing)

Status (27 Nov 2026 evening): **Partner Center account created; product
reserved as an MSIX app ("Tessera - Disk Space Analyzer and Cleaner");
real Store package builds, validates and was test-installed on the owner
laptop.** Not submitted. Submission and publication need the owner's OK.

## Decisions

* Route: **MSIX** (chosen 27 Nov). Microsoft signs the package for free and
  the Store delivers updates. The EXE/MSI route would need a paid
  certificate from a CA in the Microsoft Trusted Root Program, our own
  installer hosting, and no Store updates.
* Plain "Tessera" is taken in the Store (another developer reserved it).
  Reserved name (owner, 27 Nov): **"Tessera - Disk Space Analyzer and
  Cleaner"**. The package's display name must match it character for
  character (it is the default in make-msix.ps1). The app's Start menu name
  and window title stay "Tessera".

## Identity (Partner Center > Product identity; not secret)

| Field | Value |
|---|---|
| Package/Identity/Name | `BeyondEnterprises.Tessera-DiskSpaceAnalyzerandClea` |
| Package/Identity/Publisher | `CN=A889C9DB-0747-4E48-BAEE-F4F32691A911` |
| Package/Properties/PublisherDisplayName | `Beyond Enterprises` |

These are the defaults in `tools/make-msix.ps1`.

## Build

1. Build the program: `npx tauri build --no-bundle` in `apps\desktop` (or
   `tools\release.ps1`, which also builds the .exe installer).
   **Build in production mode** (`$env:NODE_ENV='production'`): with
   NODE_ENV=development the interface ships React's development version
   (`tools\check-production-bundle.ps1` checks; release.ps1 now does this).
2. `powershell -File tools\make-msix.ps1` writes
   `target\msix\Tessera_<version>.0_x64.msix` (unsigned; the Store signs
   it). Version = the app's version + ".0" (Store rule).

27 Nov: built with the real identity and the reserved name; makeappx
accepted the manifest; 5.2 MB.

## Test install on the owner laptop (27 Nov ~20:05 IST)

Developer Mode was turned on with the owner's permission (Settings >
System > For developers). `Add-AppxPackage -Register target\msix\layout\AppxManifest.xml`
installed it; launched from Start's app list (`shell:AppsFolder\<family>!Tessera`)
it ran with package identity (family
`BeyondEnterprises.Tessera-DiskSpaceAnalyzerandClea_txpmgv1dea9w0`); a
second launch with a folder handed the folder to the open window, which
scanned it. Files: **new** files in Tessera's AppData folders are written
to the normal location, but files that **already existed** (from the .exe
version) get a private copy when the Store app changes them (Windows'
copy-on-write for packaged apps): someone who had the .exe version sees a
separate log and Restore list in the Store version. A first cleanup failed
with "This review has expired" - not Store-specific: see the bug below.
Removed again with `Remove-AppxPackage` before rebuilding.

Bug found by this test and fixed: release builds bundled React's
development version (NODE_ENV=development leaked into `tauri build`),
which runs every screen's effects twice; the cleanup screen then checked
the list twice at once, and with the engine keeping only one check, the
answer order decided whether Move worked. Fixed three ways: production
builds (with a check), the engine keeps the last three checks, and the
screen re-checks and retries once by itself.

## What differs in the Store version

| Feature | Store version |
|---|---|
| Scanning, map, cleanup, Recycle Bin, Restore, licence (Credential Manager) | Same (full-trust desktop app, `runFullTrust`) |
| Update check | Off; the Store updates the app; Settings says so (`update::store_managed()`) |
| Right-click "Scan with Tessera" | **Not available**: added by the .exe installer only. A Store version needs a File Explorer command handler (COM, `desktop4:FileExplorerContextMenus`) - not built |
| Start at sign-in | **Not available yet**: Windows keeps a packaged app's registry writes private, so the Run entry would do nothing. Settings shows the option as not available in the Store version, and the backend refuses it. Needs the package's `windows.startupTask` extension - not built |
| Settings > Diagnostics > Show log file | Fine for Store-only users (new files go to the normal folder). If the .exe version was used before, the Store copy writes to a private copy of the old log, which File Explorer can't show |
| Settings and history files | Same folders as the .exe version for new files; files that existed before are copied privately when changed (see the test install below) |

## Testing before submission

Installing an unsigned package locally needs **Developer Mode** (on since
27 Nov with the owner's OK; Settings > System > For developers). Then
`Add-AppxPackage -Register target\msix\layout\AppxManifest.xml` installs it
next to the .exe version, and `Remove-AppxPackage` removes it. Close the
.exe version first: both share the single-instance identity.
Checklist: starts; scans a disposable folder; cleanup to the Recycle Bin and
Undo; licence activates; Settings shows "updates come from the Store" and
start at sign-in as not available; Show log file; uninstall leaves nothing.
The Windows App Certification Kit (`appcert.exe`, installed) also needs an
installed package and administrator rights; the Store runs the same tests
during certification anyway.

## Submission checklist (Partner Center, for the person submitting)

1. Apps and games > Tessera > **Start your submission**.
2. **Pricing and availability**: Free (Tessera Pro is bought inside the app,
   through Dodo Payments; non-game apps may use their own payments). If
   asked about in-app purchases, answer that the app offers an optional
   one-time Pro upgrade sold through its own checkout. Markets: all.
   Visibility: public (owner, 27 Nov: "submit, and also make sales live");
   the app is in live-sales mode from version 1.0.0.
3. **Properties**: category *Utilities & tools*; privacy policy URL
   `https://tessera-96w.pages.dev/privacy`; website
   `https://tessera-96w.pages.dev`; support contact `hullashoo6@gmail.com`.
   System requirements: Windows 11, 64-bit.
4. **Age ratings**: answer the questionnaire (no violence, user content,
   chat, location or purchases of physical goods) - expected rating 3+.
5. **Packages**: upload `target\msix\Tessera_1.0.0.0_x64.msix` (version
   1.0.0, live sales; built 27 Nov evening).
6. **Store listing (English)**: paste from `docs/STORE-LISTING.md`; add at
   least one screenshot (PNG, 1366 x 768 or larger).
7. **Submission options**: choose **"Publish manually"**, so nothing goes
   live after certification until the owner presses Publish.
8. Restricted capability `runFullTrust`: if asked why, paste the reason
   from `docs/STORE-LISTING.md`.
9. Submit for certification (up to about three business days).

## Why the Store (background)

| Route | Cost | SmartScreen |
|---|---|---|
| Direct-download `.exe`, unsigned (today) | Free | Warning on every new version |
| Paid certificate / Azure Artifact Signing | Paid | Warning until reputation builds |
| SignPath Foundation | Free | Open-source projects only (Tessera is not) |
| **Microsoft Store, MSIX package** | **Free** | **No warning**: Microsoft re-signs the package |

macOS has no free equivalent: notarisation requires the paid Apple
Developer Program. The Mac build is ad-hoc signed only.
