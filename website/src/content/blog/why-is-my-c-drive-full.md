---
title: 'Why is my C: drive full? 10 common causes on Windows 11, and safe fixes'
description: 'The usual reasons a C: drive fills up on Windows 11, from Windows.old and update files to downloads and app caches, and how to fix each one safely.'
date: 2026-11-28
---

A full C: drive slows Windows down, stops updates from installing and leaves apps with no room to work. The good news is that the space rarely disappears into one mystery file. It usually builds up in a handful of predictable places, and most of them can be cleared safely once you know which is which.

This guide goes through the ten most common causes, roughly in the order you are likely to find them, and the safe way to deal with each.

## First, see where the space actually went

Before deleting anything, get a picture of what is using the space.

1. Open **Settings > System > Storage**. Windows shows your largest categories, such as Apps, Temporary files and Documents.
2. Select **Show more categories** for the full list, and select any category to see what is inside it.

Windows' categories are broad, so a large "Other" or "Documents" figure will not tell you which folder is responsible. A disk space analyser shows every folder to scale, so the biggest ones stand out straight away. That is what [Tessera](/) does, and it is free to scan.

## 1. The previous version of Windows (Windows.old)

After a major Windows upgrade, your old installation is kept in a folder called **Windows.old** so that you can go back if something goes wrong. It is often 15 to 30 GB.

Windows deletes it on its own **10 days** after the upgrade. If you are happy with the new version and need the space sooner, go to **Settings > System > Storage > Temporary files**, tick **Previous Windows installation(s)** (on some versions it is called *Previous version of Windows*) and select **Remove files**. Once it is gone, you can no longer roll back.

Do not delete the Windows.old folder by hand in File Explorer. Parts of it are protected, and Windows' own clean-up removes it properly.

## 2. Leftover Windows Update files

Windows keeps update packages after installing them, and its Delivery Optimization feature keeps copies of updates it may share with other PCs. Both appear under **Settings > System > Storage > Temporary files** as **Windows Update Cleanup** and **Delivery Optimization Files**. They are safe to remove there.

## 3. Your Downloads folder

Downloads is where installers, videos, archives and disk images pile up for years. It is often the single easiest place to win back several gigabytes.

Sort the folder by size in File Explorer (**View > Details**, then select the **Size** column) and look at the largest items first. Installers for apps you have already installed can usually go, because you can download a newer version when you need it.

## 4. Videos, recordings and phone backups

- **Game and screen recordings.** The Xbox Game Bar saves clips to *Videos\Captures*, and the Snipping Tool saves screen recordings to *Videos\Screen Recordings*. A few minutes of high-resolution video can take gigabytes.
- **iPhone and iPad backups.** Backups made by iTunes or the Apple Devices app are stored on C: and can reach tens of gigabytes per device. Manage them from the app, not by deleting folders: in Apple Devices, open your device's settings and manage its backups there.

## 5. The Recycle Bin

Files you delete go to the Recycle Bin and still take up space until it is emptied. On a large drive, the Recycle Bin is allowed to grow to several gigabytes. Open it, check there is nothing you need, then choose **Empty Recycle Bin**.

## 6. Temporary files and app caches

Windows and your apps keep temporary files and caches to work faster. Most of them are rebuilt automatically, so clearing them is safe, but they come back over time.

- Use **Settings > System > Storage > Temporary files** for the files Windows manages.
- Clear browser caches from the browser itself, using **Clear browsing data** in its privacy settings. Choose cached files only if you want to stay signed in to websites.
- Turn on **Storage Sense** on the Storage page to clean temporary files automatically. Leave its option to clear the Downloads folder switched off unless you never keep anything there.

## 7. The hibernation file

If your PC supports hibernation, Windows keeps a file called **hiberfil.sys** in the root of C: that can be as large as a good share of your memory. If you never hibernate, you can turn hibernation off, which removes the file:

```
powercfg /hibernate off
```

Run this in **Terminal (Admin)**. Do not delete hiberfil.sys by hand; Windows manages it. Note that turning off hibernation also turns off Fast Startup, which uses the same file.

## 8. System Restore points

System Restore keeps snapshots that let you undo system changes. They are useful, but they can take a lot of space. To limit them, search the Start menu for **Create a restore point**, select your C: drive, choose **Configure** and lower the **Max Usage** slider. Keeping some space for restore points is wise; setting it to a few per cent of the drive is usually enough.

## 9. Developer tools and large apps

If you write code or work with games and creative apps, these are often the biggest users of space:

- **node_modules** folders, build output and package caches, which can all be rebuilt or downloaded again. Our [developer guide](/guides/developer-caches) explains which is which.
- **Docker, virtual machines and Android emulators**, which store their data in single large files. Use each tool's own clean-up rather than deleting files.
- **Games**, where a single modern title can take 100 GB or more. Uninstall the ones you no longer play from **Settings > Apps > Installed apps**, or from the game launcher, which can usually reinstall them later.

## 10. Files you have synced from the cloud

OneDrive can keep a local copy of every file it syncs. If your OneDrive folder is large, right-click it and choose **Free up space**. Your files stay in OneDrive and download again when you open them. Our [guide to cloud files](/guides/cloud-files) covers this in more detail.

## What you should never delete by hand

Some large files and folders look tempting but belong to Windows. Deleting them by hand can stop Windows from starting or updating.

- **C:\Windows**, including **WinSxS**. To shrink WinSxS safely, use Windows' own tool in **Terminal (Admin)**: `Dism.exe /Online /Cleanup-Image /StartComponentCleanup`.
- **pagefile.sys, swapfile.sys and hiberfil.sys** in the root of C:.
- **Program Files** and **Program Files (x86)**. Uninstall apps from **Settings > Apps** instead.
- **System Volume Information** and **$Recycle.Bin**.

## Why the numbers may not add up

Even after a full scan, the total of your folders may be less than the space Windows reports as used. Restore points, other users' files and space the system reserves cannot be seen by a folder scan. A good analyser shows that difference instead of hiding it.

## A safe routine

1. Let Windows clean what it manages: Previous version of Windows, Windows Update Cleanup and Temporary files.
2. Find the largest folders and files with a disk space analyser.
3. Move what you no longer need to the Recycle Bin rather than deleting it permanently, so a mistake can be undone.
4. Empty the Recycle Bin once you are sure.

Tessera follows the same approach. Its Smart Scan explains why each suggestion was found and how safe it is to remove, cleanup moves items to the Recycle Bin with Undo, and Windows' protected folders are always left alone.
