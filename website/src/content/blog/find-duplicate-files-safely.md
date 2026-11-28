---
title: 'How to find and remove duplicate files safely on Windows and Mac'
description: 'Why duplicate files build up, how to tell real copies from files that only look alike, and a safe way to remove extra copies without losing anything.'
date: 2026-11-28
---

Duplicate files are one of the quieter ways a drive fills up. A video downloaded twice, a photo folder imported into two places, an old backup that is really just a copy of your Documents folder: none of them is noticeable on its own, but together they can take tens of gigabytes.

Removing duplicates is also one of the easier ways to lose something by accident. This guide explains how duplicates appear, how to be sure two files really are the same, and a safe way to clear the extra copies on Windows and Mac.

## Where duplicates come from

- **Downloading the same file twice.** Browsers add a number, so you end up with *report.pdf* and *report (1).pdf*.
- **Copying instead of moving.** Dragging a folder to another drive or folder copies it, and the original stays behind.
- **Photo and phone imports.** Importing from a phone or camera more than once, or into more than one app, creates a second set of every picture and video.
- **Manual backups.** A "Backup 2023" folder made by copying your Documents folder is a full second copy of everything in it.
- **Email and chat attachments** saved to Downloads, then again to Documents.

## Same name or size does not mean same file

Two files can share a name and contain different things, such as two different *IMG_0001.jpg* photos from two phones. Two files can also have different names and be exact copies, such as *holiday.mp4* and *video (1).mp4*. Even matching sizes are not proof: two different files can be the same size down to the byte.

The only reliable test is to **compare the contents**. Good duplicate finders do this in stages, because reading every file in full would take a long time:

1. **Group files by exact size.** Files of different sizes cannot be copies, so most files are ruled out at once without being read.
2. **Compare small samples** from the start, middle and end of same-size files. Files that differ almost always differ there, so most of the rest are ruled out after reading a tiny part of each.
3. **Compare the full contents** of the files that are still left, using a hash (a fingerprint calculated from every byte). Only files whose full contents match are reported as duplicates.

If a tool only compares names, dates or sizes, treat its results as suggestions to check, not as copies to delete.

## Finding duplicates on Windows

Windows does not include a duplicate file finder. You can find likely copies by hand:

1. In File Explorer, open the folder you want to check, such as Downloads or Pictures.
2. Search for `(1)` or `- Copy` to find files Windows and browsers renamed when a copy was made.
3. Choose **View > Details** and sort by **Size** to spot files of the same size next to each other.

This catches the obvious cases, but not copies spread across different folders or with different names. For those you need a tool that compares contents.

## Finding duplicates on a Mac

- **Photos.** On macOS Ventura and later, Photos finds duplicate pictures and videos in your library by itself. Open the **Duplicates** album under **Utilities** in the sidebar, and choose **Merge** to keep the best copy with all its details. Always remove duplicate photos this way, inside the Photos app, rather than in Finder.
- **Finder.** For other files, open a folder, choose **View > as List** and sort by **Size**, or search for "copy" to find files Finder renamed when duplicating them.

As on Windows, this only finds the obvious cases.

## Removing duplicates safely

Whichever tool you use, these rules prevent the usual mistakes.

**Keep at least one copy.** It sounds obvious, but when you select copies in bulk it is easy to tick every one. A good tool warns you when all copies of a file are selected.

**Decide which copy to keep by where it lives, not just its date.** Keep the copy in the place you actually use, such as your Pictures library rather than an old Downloads folder. When in doubt, keeping the oldest copy is a sensible default, since it is usually the original.

**Leave app libraries to their apps.** Photos libraries, music libraries, Lightroom catalogues and similar folders are managed by their apps. Deleting files inside them by hand can break the library. Use the app's own duplicate tools instead.

**Be careful with program and system folders.** Apps often ship identical files, such as the same component in two programs. They are copies, but each app needs its own. Only remove duplicates from your own folders: documents, downloads, pictures, videos and music.

**Watch out for cloud folders.** In OneDrive, iCloud Drive or Dropbox, deleting a copy also deletes it from the cloud and your other devices. That is fine for a genuine duplicate, but check you are removing the copy you mean to.

**Move copies to the Recycle Bin or Trash, not straight to permanent deletion.** Live with the result for a few days, then empty the bin once you are sure nothing is missing.

## How Tessera finds duplicates

Tessera's duplicate finder works as described above. It groups files of 1 MB and larger by size, compares samples, then compares the full contents, and lists only exact copies. It works on both Windows and Mac, reads files on your computer only, and never opens cloud files that are stored online, so nothing is downloaded.

For each group of copies you can keep the oldest or the newest, and Tessera warns you if every copy is selected. Chosen copies go to the Recycle Bin or Trash with Undo, so a wrong choice is easy to put right. The search shows how far it has got and how much it has found so far, and it carries on while you use the rest of the app.
