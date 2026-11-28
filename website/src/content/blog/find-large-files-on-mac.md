---
title: 'How to find large files on a Mac, and decide what is safe to delete'
description: 'Four ways to find the biggest files and folders on your Mac, from Storage settings and Finder to Terminal, plus what to keep and what you can safely remove.'
date: 2026-11-28
---

When your Mac says its disk is almost full, the quickest win is usually a few very large files you forgot about: an old video export, a disk image you downloaded once, or a backup of a phone you no longer own. The hard part is finding them.

Here are four ways to find large files on a Mac, from the simplest to the most thorough, followed by a short guide to what is safe to remove.

## 1. Storage settings: the built-in overview

macOS has a storage overview that lists your largest documents.

1. Open the Apple menu, then **System Settings > General > Storage**.
2. Wait for the bar at the top to finish calculating. It shows how much space each category uses.
3. Next to **Documents**, select the **Info** button (the small "i").
4. Choose **Large Files** to see your biggest documents, largest first. **Downloads** shows what is in your Downloads folder, and **File Browser** lets you explore folders by size.

From this window you can select a file and choose **Show in Finder** to see where it lives, or **Delete** to remove it. Be aware that deleting from here removes files straight away rather than moving them to the Trash, so check each one first.

The other categories, such as **Applications**, **iOS Files** and **Photos**, have their own Info buttons too. **iOS Files** is worth a look: it lists old iPhone and iPad backups and software updates, which can take many gigabytes each.

## 2. Finder search: large files anywhere on your Mac

Finder can search by file size, which catches large files the Storage window does not show.

1. Open a Finder window and press **Command-F**.
2. Choose **This Mac** at the top of the search.
3. Select the first search criterion (it usually says **Kind**) and change it to **Other…**.
4. In the list that appears, tick **File Size** and select **OK**.
5. Set the rule to **File Size is greater than 1 GB** (you can change the number and unit).

Finder now lists every file over 1 GB. Choose **View > as List** and select the **Size** column to sort them. To keep the search for later, choose **Save** and give it a name; it appears in the Finder sidebar.

## 3. Terminal: for when you want exact numbers

If you are comfortable with Terminal, two commands give quick, exact answers. Both only read information; neither changes anything.

To list the size of each folder in your home folder, smallest to largest:

```
du -sh ~/* 2>/dev/null | sort -h
```

To list every file over 1 GB in your home folder:

```
find ~ -type f -size +1G 2>/dev/null
```

macOS may ask whether Terminal can access folders such as Desktop and Documents. Allow it if you want those folders included.

## 4. A disk space analyser: everything at once

The methods above find big *files*, but space is often taken by folders full of medium-sized files, such as a photo export, a project folder or an app's cache. A disk space analyser adds up every folder and shows the result as a map, so the folders taking the most space stand out at a glance.

[Tessera](/) does this on both Mac and Windows. It reads your files on your Mac only, never downloads iCloud files that are stored online, and lists your largest files alongside the map.

## What is usually safe to remove

Once you know what is large, decide what can go. These are usually safe:

- **Disk images and installers** in Downloads: files ending in .dmg or .pkg for apps you have already installed. You can download them again if needed.
- **"Install macOS …" apps** in Applications, left over after an upgrade. They can be downloaded again from the App Store.
- **Old iPhone and iPad backups** of devices you no longer use. Delete them from **System Settings > General > Storage > iOS Files**, or in Finder by selecting the device and choosing **Manage Backups**.
- **Screen recordings and video exports** you have already shared or saved elsewhere. macOS saves screen recordings to the Desktop by default.
- **Apps you no longer use.** Drag them from Applications to the Trash, or use the app's own uninstaller if it has one.
- **Developer data** such as Xcode's DerivedData and old simulators, which our [developer guide](/guides/developer-caches) covers in detail.

## What to leave alone

- **The System folder, /Library and anything macOS protects.** They are not where your space went, and macOS manages them.
- **Inside your Photos and Music libraries.** These look like single files in Finder but are packages managed by their apps. Delete photos or songs from inside the Photos or Music app instead, or the library can be damaged.
- **System Data.** This category holds caches, logs and other files macOS manages, and it shrinks on its own when space is needed. There is no safe way to clear it by hand, and most "clean System Data" tips do more harm than good.
- **Files in iCloud Drive that you still need.** Deleting a file from iCloud Drive deletes it from all your devices. To free space but keep the file, right-click it in Finder and choose **Remove Download**. See our [guide to cloud files](/guides/cloud-files).

## Delete in a way you can undo

Move files to the Trash rather than deleting them immediately, so a mistake can be put back. Space is only freed when the Trash is emptied, so empty it once you are sure. In **System Settings > General > Storage** you can also turn on **Empty Trash Automatically**, which removes items that have been in the Trash for 30 days.

Tessera works the same way: you review what you have chosen, it moves those items to the Trash, and Undo or its Restore page puts them back if you change your mind.
