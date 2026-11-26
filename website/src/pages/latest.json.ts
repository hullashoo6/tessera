import type { APIRoute } from 'astro'
import { latest } from '../data/site'

// Read by the app's optional update check. Public; contains no user data.
export const GET: APIRoute = () =>
  new Response(JSON.stringify({ version: latest.version }), { headers: { 'Content-Type': 'application/json' } })
