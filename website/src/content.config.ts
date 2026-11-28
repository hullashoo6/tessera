import { defineCollection } from 'astro:content'
import { glob } from 'astro/loaders'
import { z } from 'astro/zod'

// Blog articles: one Markdown file each in src/content/blog. The file name
// is the address (why-is-my-c-drive-full.md -> /blog/why-is-my-c-drive-full).
// How to add one: docs/BLOG.md.
const blog = defineCollection({
  loader: glob({ pattern: '**/*.md', base: './src/content/blog' }),
  schema: z.object({
    title: z.string().max(110),
    /** One or two sentences for search results and the blog list. */
    description: z.string().min(50).max(200),
    date: z.coerce.date(),
    updated: z.coerce.date().optional(),
    /** Hidden from the site until set to false. */
    draft: z.boolean().default(false),
  }),
})

export const collections = { blog }
