import { defineConfig } from 'astro/config'

// The production domain has not been chosen yet. `.invalid` is reserved and
// can never resolve, so nothing can accidentally point at a real site.
// Set PUBLIC_SITE_URL when a domain is approved.
const site = process.env.PUBLIC_SITE_URL ?? 'https://tessera.invalid'

export default defineConfig({
  site,
  output: 'static',
  trailingSlash: 'ignore',
  build: { inlineStylesheets: 'auto' },
  image: { responsiveStyles: true },
})
