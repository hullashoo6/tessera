// Screenshots the Tessera development window's page with emulated system
// settings (high contrast, dark mode, ...) through the WebView2 DevTools
// protocol, so they can be checked without changing the computer's settings.
// Only the app's own page is captured, never the rest of the screen.
//
// Start the dev app with
//   $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9333 --remote-allow-origins=*'
// then:
//   node tools/cdp-shot.mjs target\shot.png "prefers-contrast=more" "prefers-color-scheme=dark"
import { writeFileSync } from 'node:fs'

const [out = 'target/cdp-shot.png', ...pairs] = process.argv.slice(2)
const features = pairs.map((p) => {
  const [name, value] = p.split('=')
  return { name, value }
})
const pages = await (await fetch('http://127.0.0.1:9333/json')).json()
const page = pages.find((p) => p.type === 'page' && p.url.startsWith('http://localhost:5173'))
if (!page) throw new Error('Tessera dev page not found on port 9333')

const ws = new WebSocket(page.webSocketDebuggerUrl)
let next = 1
const pending = new Map()
ws.onmessage = (e) => {
  const msg = JSON.parse(e.data)
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg)
    pending.delete(msg.id)
  }
}
const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const id = next++
    pending.set(id, (m) => (m.error ? reject(new Error(m.error.message)) : resolve(m.result)))
    ws.send(JSON.stringify({ id, method, params }))
  })
await new Promise((r) => (ws.onopen = r))
await send('Emulation.setEmulatedMedia', { features })
await new Promise((r) => setTimeout(r, 1200)) // let the app redraw
const { data } = await send('Page.captureScreenshot', { format: 'png' })
writeFileSync(out, Buffer.from(data, 'base64'))
await send('Emulation.setEmulatedMedia', { features: [] })
ws.close()
console.log(`saved ${out} with ${pairs.join(', ') || 'no emulation'}`)
