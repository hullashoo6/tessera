// Answers describe the product as it is today. Update with STATUS.md.
export const faqs: [string, string][] = [
  [
    'How does Tessera work?',
    'It reads the names and sizes of the files on a drive or in a folder, adds them up folder by folder, and draws the result as a map where each tile is sized by the space it uses. Results appear while it reads, so you can start exploring straight away.',
  ],
  [
    'Which computers does it run on?',
    'Windows 11 (64-bit) and Macs. One Mac download works on both Apple silicon and Intel Macs. Each version is listed in the download section only after it passes testing on real machines.',
  ],
  [
    'What is free, and what does Pro add?',
    'Free, with no limits: scanning, the storage map, largest files, search, Smart Scan suggestions, the duplicate finder, the list of installed apps and Restore. Tessera Pro adds the actions: cleaning up to the Recycle Bin or Trash in a few clicks, freeing space from OneDrive and iCloud files, uninstalling apps from Tessera, CSV reports and storage history.',
  ],
  [
    'Is my data private?',
    'Yes. Scans happen entirely on your computer, and file names, folders and scan results are never sent anywhere. Tessera has no analytics or tracking. It connects to the internet only to check a Pro licence with our payment provider and, if you allow it, to see whether a new version is available.',
  ],
  [
    'Can Tessera delete my files?',
    'Never by itself. You choose items and review them first, and Tessera then moves them to the Recycle Bin or Trash. It never deletes anything permanently, and system folders are protected.',
  ],
  [
    'Can I undo a cleanup?',
    'Yes. The Restore page lists everything Tessera moved in the last 30 days, and Put back returns an item to where it was. It never replaces anything that is already there.',
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
    'Run the installer for your system. To uninstall on Windows, use Settings > Apps; on a Mac, move Tessera from Applications to the Trash. Your files are not affected either way.',
  ],
]
