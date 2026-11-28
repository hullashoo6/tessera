# Blog

Live at **https://tessera-96w.pages.dev/blog** (started 28 Nov 2026; the owner
plans three articles a day). Listed in the site menu, footer, `sitemap.xml`
and the RSS feed `/blog/rss.xml`.

## Adding an article

1. Create `website/src/content/blog/<address>.md`. The file name becomes the
   address: `find-large-files-on-mac.md` -> `/blog/find-large-files-on-mac`.
   Use lower case and hyphens, and words people search for.
2. Start the file with this header (checked when the site is built; the build
   fails with a clear message if something is missing or too long):

   ```
   ---
   title: 'How to …'                 # at most 110 characters
   description: 'One or two sentences for Google and the blog list.'   # 50-200 characters
   date: 2026-11-29                  # the publishing day
   updated: 2026-12-05               # optional, when an article is revised
   draft: true                       # optional: hidden from the site until removed
   ---
   ```

3. Write the article in Markdown below the header: `## Heading`, `- list`,
   `**bold**`, `[link](/guides/cloud-files)`, and code in backticks.
4. Build and deploy the website (docs/WEBSITE.md) and check the page with
   `node tools/site-shot.mjs https://tessera-96w.pages.dev/blog/<address> target/blog.png 390 844 mobile 1`.

## Writing rules (owner: no unprofessional copy)

- Genuinely useful, accurate how-to articles about storage on Windows and Mac.
  Check every menu path and command against Microsoft's or Apple's own help
  pages, and use the exact labels shown on screen (Microsoft's "Delivery
  Optimization" keeps its z even though the site is otherwise British English).
- British English, plain words, short paragraphs. No hype, no invented
  statistics, no claims about competitors.
- Mention Tessera briefly and honestly at the end; describe only features the
  released version has (STATUS.md).
- No personal paths, usernames or screenshots of real files.
- Google ranks thin or mass-produced articles down: each article must answer
  one real question better than what is already out there.

## Published

| Date | Address | Topic |
|---|---|---|
| 28 Nov 2026 | why-is-my-c-drive-full | 10 causes of a full C: drive on Windows 11 and safe fixes |
| 28 Nov 2026 | find-large-files-on-mac | Four ways to find large files on a Mac; what to keep |
| 28 Nov 2026 | find-duplicate-files-safely | How duplicates arise, content comparison, safe removal |

Ideas not yet written: what "System Data" is on a Mac; WinSxS explained; how
to move games to another drive; Recycle Bin size limits; free up iCloud space;
why a new laptop's 256 GB fills so fast; cleaning Downloads safely; clearing
browser caches in each browser; iPhone backups on Windows and Mac; Docker
disk use; node_modules across many projects; Windows.old explained.
