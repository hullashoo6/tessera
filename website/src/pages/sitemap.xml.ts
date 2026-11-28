import type { APIRoute } from 'astro'
import { posts } from '../data/blog'

const pages = [
  '/',
  '/help',
  '/blog',
  '/guides',
  '/guides/free-up-space-windows',
  '/guides/free-up-space-mac',
  '/guides/developer-caches',
  '/guides/cloud-files',
  '/changelog',
  '/privacy',
  '/terms',
]

export const GET: APIRoute = async ({ site }) => {
  const articles = (await posts()).map((p) => ({
    path: `/blog/${p.id}`,
    lastmod: (p.data.updated ?? p.data.date).toISOString().slice(0, 10),
  }))
  const urls = [
    ...pages.map((p) => `  <url><loc>${new URL(p, site).href}</loc></url>`),
    ...articles.map((a) => `  <url><loc>${new URL(a.path, site).href}</loc><lastmod>${a.lastmod}</lastmod></url>`),
  ].join('\n')
  const body = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}\n</urlset>\n`
  return new Response(body, { headers: { 'Content-Type': 'application/xml' } })
}
