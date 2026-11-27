// Answers describe the product as it is today. Update with STATUS.md.
export const faqs: [string, string][] = [
  [
    'How does Tessera work?',
    'It reads the names and sizes of the files on a drive or in a folder, adds them up folder by folder, and draws the result as a map where each tile is sized by the space it uses. Results appear while it reads, so you can start exploring straight away.',
  ],
  [
    'Which computers does it run on?',
    'Windows 11 (64-bit), and Macs with macOS 12 or later. One Mac download works on both Apple silicon and Intel Macs. Windows and Mac have the same features, and one Pro licence works on either.',
  ],
  [
    'What is free, and what does Pro add?',
    'Free, with no limits: scanning, the storage map, largest files, search, Smart Scan suggestions, the duplicate finder, the list of installed apps and Restore. Tessera Pro adds the actions: cleaning up to the Recycle Bin or Trash in a few clicks, freeing space from OneDrive and iCloud files, uninstalling apps from Tessera, storage history and reports.',
  ],
  [
    'Is my data private?',
    'Yes. Scans happen entirely on your computer, and file names, folders and scan results are never sent anywhere. Tessera has no analytics or tracking. It connects to the internet only to check a Pro licence with our payment provider and, if you allow it, to see whether a new version is available.',
  ],
  [
    'Can I scan a folder straight from File Explorer or Finder?',
    'Yes. Right-click a folder or drive and choose Scan with Tessera in File Explorer (on Windows 11 it is under Show more options), or Open With > Tessera in Finder. You can also drop a folder onto the Tessera window. If Tessera is already open, it scans the new folder in the same window.',
  ],
  [
    'Can Tessera delete my files?',
    'Never by itself. You add items to a cleanup list and review it first, and Tessera then moves them to the Recycle Bin or Trash. Anything too big for the Recycle Bin is left where it is, never deleted in its place. Tessera deletes something permanently only if you choose Delete permanently and confirm a separate warning. System folders are protected.',
  ],
  [
    'Can I undo a cleanup?',
    'Yes. Right after a cleanup, Undo puts everything back in one click. The Restore page lists everything Tessera moved in the last 30 days, and Put back returns an item to where it was. It never replaces anything that is already there.',
  ],
  [
    'What if a file cannot be removed?',
    'Tessera tells you why in plain words and what to do: for example which app has the file open, so you can close it and choose Try again, or that it needs administrator permission. For a folder where an app keeps a few files open, such as a browser cache, it can move everything else and leave just the open files.',
  ],
  [
    'Will freeing space from cloud files delete them?',
    'No. The file stays in your OneDrive or iCloud Drive and downloads again when you open it. Only the copy on this computer is removed. Files set to always stay on this device are left alone.',
  ],
  [
    'How does the duplicate finder decide two files are the same?',
    'It compares their full contents, not just names or sizes, so only exact copies are listed. You choose which copy to keep.',
  ],
  [
    'Why might the total differ from what my drive reports?',
    'Your drive also holds things no folder scan can see, such as system restore points, other users’ files and space the system reserves. Tessera shows the difference instead of hiding it, and labels any size it has to estimate.',
  ],
  [
    'Does it work offline?',
    'Yes. Scanning and every free feature work without an internet connection. A Pro licence keeps working offline for 30 days at a time between checks.',
  ],
  [
    'Can I get a refund?',
    'Yes. You can ask for a full refund within 14 days of buying Tessera Pro, for any reason. Email us with your order number.',
  ],
  [
    'How do I install and uninstall it?',
    'On Windows, run the installer; on a Mac, open the download and drag Tessera to Applications. To uninstall on Windows, use Settings > Apps; on a Mac, move Tessera from Applications to the Trash. Your files are not affected either way.',
  ],
]

/** The questions shown on the home page; the Help page shows them all. */
export const homeFaqs = faqs.filter(([q]) =>
  [
    'How does Tessera work?',
    'Which computers does it run on?',
    'What is free, and what does Pro add?',
    'Is my data private?',
    'Can I undo a cleanup?',
    'Can I get a refund?',
  ].includes(q),
)
