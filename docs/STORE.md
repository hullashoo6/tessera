# Microsoft Store (free Windows signing)

Status (27 Nov 2026 evening): **Partner Center account created; product
reserved as an MSIX app; real Store package builds and validates.** Not
installed or submitted yet. Submission and publication need the owner's OK.

## Decisions

* Route: **MSIX** (chosen 27 Nov). Microsoft signs the package for free and
  the Store delivers updates. The EXE/MSI route would need a paid
  certificate from a CA in the Microsoft Trusted Root Program, our own
  installer hosting, and no Store updates.
* Plain "Tessera" is taken in the Store (another developer reserved it).
  The reserved name is longer (the identity name below shows its start:
  "Tessera - Disk Space Analyzer and Clea..."). **The exact reserved name
  is needed**: the package's display name must match it character for
  character. The app's Start menu name and window title stay "Tessera".

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
2. `powershell -File tools\make-msix.ps1 -StoreName "<exact reserved name>"`
   writes `target\msix\Tessera_<version>.0_x64.msix` (unsigned; the Store
   signs it). Version = the app's version + ".0" (Store rule).

27 Nov: built with the real identity and the provisional name "Tessera -
Disk Space Analyzer and Cleaner" (to be replaced by the exact reserved
name); makeappx accepted the manifest; 5.2 MB.

## What differs in the Store version

| Feature | Store version |
|---|---|
| Scanning, map, cleanup, Recycle Bin, Restore, licence (Credential Manager) | Same (full-trust desktop app, `runFullTrust`) |
| Update check | Off; the Store updates the app; Settings says so (`update::store_managed()`) |
| Right-click "Scan with Tessera" | **Not available**: added by the .exe installer only. A Store version needs a File Explorer command handler (COM, `desktop4:FileExplorerContextMenus`) - not built |
| Start at sign-in | **Not available yet**: Windows keeps a packaged app's registry writes private, so the Run entry would do nothing. Settings shows the option as not available in the Store version, and the backend refuses it. Needs the package's `windows.startupTask` extension - not built |
| Settings > Diagnostics > Show log file | To check: Windows redirects a packaged app's AppData writes into the package's private folder, so File Explorer may not find the log at the normal path |
| Settings and history files | Kept in the package's private AppData (Windows does this automatically); removed when the Store app is uninstalled |

## Testing before submission (needs the owner)

Installing an unsigned package locally needs **Developer Mode** (Settings >
System > For developers > Developer Mode: on; it can be turned off again
afterwards). Then `Add-AppxPackage -Register target\msix\layout\AppxManifest.xml`
installs it next to the .exe version, and `Remove-AppxPackage` removes it.
Checklist: starts; scans a disposable folder; cleanup to the Recycle Bin and
Undo; licence activates; Settings shows "updates come from the Store" and
start at sign-in as not available; Show log file; uninstall leaves nothing.
The Windows App Certification Kit (`appcert.exe`, installed) also needs an
installed package and administrator rights; the Store runs the same tests
during certification anyway.

## Submission checklist (Partner Center, for the person submitting)

1. Apps and games > Tessera > **Start your submission**.
2. **Pricing and availability**: Free (Tessera Pro is bought inside the app,
   through Dodo Payments; non-game apps may use their own payments).
   Markets: all. Visibility: choose **"Hide this product in the Store"** or
   private audience until launch if the owner wants a quiet first release.
3. **Properties**: category *Utilities & tools*; privacy policy URL
   `https://tessera-96w.pages.dev/privacy`; website
   `https://tessera-96w.pages.dev`; support contact `hullashoo6@gmail.com`.
   System requirements: Windows 11, 64-bit.
4. **Age ratings**: answer the questionnaire (no violence, user content,
   chat, location or purchases of physical goods) - expected rating 3+.
5. **Packages**: upload `Tessera_0.1.0.0_x64.msix`.
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
