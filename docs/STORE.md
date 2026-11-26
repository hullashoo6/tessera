# Free code signing via the Microsoft Store

Status: **prepared, not tested**. Needs a Partner Center account, which is a
new external account and so needs the owner.

## Why

| Route | Cost | SmartScreen |
|---|---|---|
| Direct-download `.exe`, unsigned (today) | Free | Warning on every new version |
| Paid certificate / Azure Artifact Signing | Paid | Warning until reputation builds |
| SignPath Foundation | Free | Open-source projects only (Tessera is not) |
| **Microsoft Store, MSIX package** | **Free** | **No warning**: Microsoft re-signs the package |

Microsoft Store registration is free for individuals (since Sept 2025) and
companies (since May 2026). Non-game apps may keep their own payments (Dodo
Payments) and 100% of revenue. Source: Microsoft Learn, "Code signing
options" and "Open a developer account".

macOS has no free equivalent: notarisation requires the paid Apple
Developer Program. The Mac build is ad-hoc signed only (required to run on
Apple silicon); downloaded copies are blocked by Gatekeeper until notarised.

## Owner steps (about 15 minutes, free)

1. Go to storedeveloper.microsoft.com, choose **Get started for free**.
   Choose **Company** to publish as Beyond Enterprises, or **Individual**.
2. In Partner Center, reserve the app name (for example "Tessera").
3. Open *Product identity* and send the developer team three values:
   `Package/Identity/Name`, `Package/Identity/Publisher` (starts `CN=`), and
   `Package/Properties/PublisherDisplayName`.

## Developer steps once the identity exists

1. `powershell -File tools\make-msix.ps1 -Name <Identity/Name> -Publisher "<CN=...>" -PublisherDisplayName "<name>"`
2. Validate with the Windows App Certification Kit.
3. Test-install on a clean Windows account; repeat the installed-build
   checklist (scan, cleanup with disposable files, licence activation).
4. Submit in Partner Center. **Publication requires owner approval.**

## Notes for MSIX

* Tessera runs as a full-trust desktop app (`runFullTrust`), so scanning,
  the Recycle Bin and Credential Manager behave as in the `.exe` build.
* The Store installs and updates the app; the website would link to the
  Store listing instead of (or as well as) the `.exe`.
