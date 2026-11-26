# Website deployment

Live at **https://tessera-96w.pages.dev** (Cloudflare Pages project `tessera`, owner's Cloudflare account, free plan). Deployed 26 Nov 2026.

To publish a new version (needs `npx wrangler login` once on the machine):

```powershell
cd website
$env:PUBLIC_SITE_URL = 'https://tessera-96w.pages.dev'
npx astro build
npx wrangler pages deploy dist --project-name tessera --branch main
```

Downloads stay hidden until `src/data/releases.json` lists verified installers with public URLs. Purchase links stay hidden until `pricing.onSale` is true.
