import { getCollection, type CollectionEntry } from 'astro:content'

export type Post = CollectionEntry<'blog'>

/** Published articles, newest first. */
export async function posts(): Promise<Post[]> {
  const all = await getCollection('blog', (p) => !p.data.draft)
  return all.sort((a, b) => b.data.date.getTime() - a.data.date.getTime() || a.id.localeCompare(b.id))
}

export const formatDate = (d: Date) =>
  new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'long', year: 'numeric', timeZone: 'UTC' }).format(d)

/** Minutes to read, at about 220 words a minute. */
export const readingMinutes = (body: string | undefined) =>
  Math.max(1, Math.round((body ?? '').split(/\s+/).filter(Boolean).length / 220))
