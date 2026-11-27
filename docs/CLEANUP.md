# Cleanup: plan to make removing files effortless and dependable

Owner brief (27 Nov 2026): removing things must be crystal clear and simple:
add to cleanup, then remove from there. Files sometimes are not removed (for
example because they are open); that must be handled well. Large cleanup
lists need Select all and similar controls.

## Where it stands (27 Nov 2026)

* "Add to cleanup" exists in the storage map menu, Largest files, Search,
  Smart Scan (one-click "add everything recreated automatically") and
  Duplicates (keep oldest/newest).
* The Cleanup page reviews the chosen items (protection rules, links,
  unscanned folders), lists every item with a Remove button, and moves them
  to the Recycle Bin / Trash one by one after a confirmation. Nothing is ever
  deleted permanently.
* Problems found:
  1. **No progress, no stop.** Measured on the owner laptop: 49.6 ms per item
     (200 files in 9.9 s), so 1,000 items take ~50 s and 10,000 take ~8 min
     behind a "Moving..." button.
  2. **Failures are unexplained.** The reason is the system's raw text
     ("The process cannot access the file because it is being used by
     another process"), with no hint which app, what to do, or a way to try
     again. Failed items drop out of the list.
  3. **A folder with one open file fails completely** (for example a browser
     cache while the browser is open).
  4. **No selection tools.** Every change to the list triggers a fresh check
     of all items; there is no Select all, filter or sort; the list is not
     virtualised, so thousands of items would slow the window.
  5. **"Where did my space go?"** Items in the Recycle Bin still use the
     space until it is emptied; the result screen only says so in a sentence.
  6. **No undo** after a cleanup except one item at a time on Restore.
  7. Items that had already disappeared count as failures.

## Why removals fail (research)

| Cause | Windows | macOS | What Tessera will do |
|---|---|---|---|
| File open in an app | Sharing violation; Restart Manager (`RmGetList`) names the app holding it, as File Explorer's "File in use" dialog does | Rare (Unix lets open files be moved) | Say which app: "In use by Microsoft Word. Close it, then Try again." |
| Folder with some files open | Whole folder refused | - | Offer "Move everything except the files in use" |
| Permission | Access denied (another user's files, admin-only places) | Not permitted (owned by root, needs a password in Finder; privacy-protected folders need Full Disk Access) | Plain reason + Show in File Explorer / Finder (+ Privacy settings link on Mac) |
| Too big for the Recycle Bin, or drive has none (USB, network) | Tessera refuses rather than let Windows delete permanently | Volumes without Trash | Explain; offer Delete permanently (if the owner agrees, below) |
| Changed after it was chosen | Re-check before each move refuses it | same | "It changed after you chose it" + Check again |
| Already gone | Not found | same | Counted as done ("already gone"), not a failure |
| Path too long | Recycle Bin limit | - | Explain; Show in File Explorer |
| Cloud file and the cloud app is not running | OneDrive error | - | "Start OneDrive, then Try again" |
| Locked (Get Info > Locked) | - | Locked flag | Explain how to unlock |

Sources: Microsoft's Restart Manager documentation and Raymond Chen's
"How do I find out which process has a file open?" (The Old New Thing);
IFileOperation / IFileOperationProgressSink documentation (per-item results
through PostDeleteItem; an error from the sink cancels the rest of a batch);
Apple developer forums and support threads on Full Disk Access protecting
~/.Trash on recent macOS.

## The new experience

**One place, three steps: Add, Review, Remove.**

1. *Add* from anywhere: map menu, Largest files, Search (+ "Add all
   results"), Smart Scan, Duplicates, and the Delete key
   (Cmd+Delete on a Mac) on a selected item. A short message confirms "Added
   to cleanup (N on the list)"; pressing the key again takes it off. The
   sidebar shows the count.
2. *Review* on the Cleanup page, a list built for thousands of items:
   checkbox per row (all ticked), Select all / none, filter by name or
   folder, sort (largest, name, folder), "Remove from list" for the selected
   rows, a running total of what is selected. Items Tessera will not touch
   (protected, links, unscanned) are in a collapsed section explaining why.
3. *Remove* with one clear button: "Move 1,204 items (4.2 GB) to the Recycle
   Bin". One short confirmation. Then a progress bar (count and size) with
   Stop.

**Results that tell you what happened and what to do next.**

* "Moved 1,201 items (4.2 GB) to the Recycle Bin", with **Undo** (puts every
  item from this cleanup back) and **Open Recycle Bin**, plus the honest
  note that the space returns when the Recycle Bin is emptied.
* Items that could not be moved stay in the cleanup list, grouped by reason,
  each group with the fix in plain words and a button: Try again, Show in
  File Explorer / Finder, Move everything except the files in use.
* "2 were already gone" shown as information, not an error.

**Fast and never laggy.** Moves run off the window thread with progress
events; Windows moves are batched (several items per shell operation, with
a per-item fallback whenever the Recycle Bin refuses one) to cut the 50 ms
per item; the list is virtualised; selection changes do not re-check files.

## Decision (owner, 27 Nov 2026 ~18:45 IST): Delete permanently is allowed

The Recycle Bin / Trash stays the default. **Delete permanently** is a
second, clearly warned choice (frees the space immediately, cannot be
undone), also offered for items the Recycle Bin cannot hold. The website's
"never deletes anything permanently" becomes "never without asking".
Permanent deletion gets the same protection rules and re-checks as moving,
its own confirmation, and is recorded on the Restore page as deleted.

## Done and verified (27 Nov 2026 evening)

All five work-plan steps below are done. Verified on the owner laptop (Windows 11, dev build) with a disposable fixture and a hidden program holding two files open: the walkthrough, numbers and screenshots are in STATUS.md (Cleanup row). Measured: 264 items moved in 4.7 s including the re-check and screen updates (was ~50-70 ms per item, now ~18 ms).

## Still open

* macOS on real hardware: Trash moves, permission/locked wording, Full Disk Access hint, permanent deletion, Open Trash.
* Windows: an item really too large for the Recycle Bin (needs a file bigger than the bin's limit), a real permission refusal (another user's file), a long-path item, OneDrive not running. The causes are mapped and unit-tested; only "in use" was produced for real.
* Keep the cleanup list across rescans (re-find items by path in the new scan) instead of starting empty.
* Name the app in the Mac "in use" case if macOS ever refuses a move because of an open file (rare).
* Microsoft Store (MSIX) build: the Recycle Bin and Restart Manager work there too; to be checked once the package can be installed.

## Work plan

1. Engine (`crates/cleanup`): structured failure kinds; in-use detection
   (Restart Manager) incl. locked files inside folders; "everything except
   files in use"; batched moves with progress and stop; undo of a whole
   cleanup; permanent delete if approved; tests with real locked files.
2. Backend commands: review once, confirm a chosen subset, progress events,
   stop, undo, open Recycle Bin / Trash.
3. Cleanup page rebuilt as above; bulk add in Largest files and Search;
   Delete-key shortcut; toast with Undo.
4. Tests: engine unit tests, real Recycle Bin round trips, a file held open
   by the test itself, screen tests for selection, progress, results.
5. Verify on the owner laptop with disposable files (dev build, then the
   installed build through tools\release.ps1).
