import type { APIRoute } from 'astro'
import { posts } from '../../data/blog'

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

/** RSS feed of the blog, newest first. */
export const GET: APIRoute = async ({ site }) => {
  const list = await posts()
  const items = list
    .map((p) => {
      const link = new URL(`/blog/${p.id}`, site).href
      return `    <item>
      <title>${esc(p.data.title)}</title>
      <link>${link}</link>
      <guid isPermaLink="true">${link}</guid>
      <pubDate>${p.data.date.toUTCString()}</pubDate>
      <description>${esc(p.data.description)}</description>
    </item>`
    })
    .join('\n')
  const body = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>The Tessera blog</title>
    <link>${new URL('/blog', site).href}</link>
    <description>Practical guides to understanding what fills your storage and getting space back safely on Windows and Mac.</description>
    <language>en-gb</language>
${items}
  </channel>
</rss>
`
  return new Response(body, { headers: { 'Content-Type': 'application/rss+xml; charset=utf-8' } })
}
