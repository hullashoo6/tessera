// Full-page screenshots of a web page in headless Microsoft Edge, with real
// device emulation, plus a check that nothing is wider than the screen.
//   node tools/site-shot.mjs <url> <out.png> [width] [height] [mobile] [scale]
// e.g. node tools/site-shot.mjs http://localhost:4321/ target/m.png 390 844 mobile 2
import { spawn } from 'node:child_process'
import { writeFileSync, mkdtempSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const [url, out, w = '1440', h = '900', mode = 'desktop', scale = '1'] = process.argv.slice(2)
const edge = 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe'
const port = 9555 + Math.floor(Math.random() * 300)
const profile = mkdtempSync(join(tmpdir(), 'tessera-shot-'))
const browser = spawn(edge, ['--headless=new', '--disable-gpu', '--hide-scrollbars', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, 'about:blank'], { stdio: 'ignore' })
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

let target
for (let i = 0; i < 40 && !target; i++) {
  await sleep(250)
  try {
    const list = await (await fetch(`http://127.0.0.1:${port}/json`)).json()
    target = list.find((t) => t.type === 'page')
  } catch {}
}
if (!target) throw new Error('Edge did not start')
const ws = new WebSocket(target.webSocketDebuggerUrl)
await new Promise((r) => ws.addEventListener('open', r, { once: true }))
let id = 0
const pending = new Map()
ws.addEventListener('message', (e) => {
  const m = JSON.parse(e.data)
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m)
    pending.delete(m.id)
  }
})
const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const n = ++id
    pending.set(n, (m) => (m.error ? reject(new Error(m.error.message)) : resolve(m.result)))
    ws.send(JSON.stringify({ id: n, method, params }))
  })
const evaluate = async (expr) => (await send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true })).result.value

const mobile = mode === 'mobile'
await send('Emulation.setDeviceMetricsOverride', { width: Number(w), height: Number(h), deviceScaleFactor: Number(scale), mobile })
if (mobile) await send('Emulation.setUserAgentOverride', { userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1' })
await send('Page.enable')
await send('Page.navigate', { url })
await sleep(2500)
// Load lazy images by scrolling through the page.
await evaluate(`(async () => { for (let y = 0; y < document.body.scrollHeight; y += innerHeight / 2) { scrollTo(0, y); await new Promise(r => setTimeout(r, 120)) } scrollTo(0, 0) })()`)
await sleep(1200)
const overflow = await evaluate(`(() => {
  const vw = document.documentElement.clientWidth
  const wide = [...document.querySelectorAll('body *')].filter(el => { const r = el.getBoundingClientRect(); return r.right > vw + 1 && getComputedStyle(el).position !== 'fixed' })
  return { viewport: vw, page: document.documentElement.scrollWidth, wider: wide.slice(0, 6).map(el => el.tagName.toLowerCase() + (el.className && typeof el.className === 'string' ? '.' + el.className.split(' ').join('.') : '') + ' right=' + Math.round(el.getBoundingClientRect().right)) }
})()`)
const height = await evaluate('document.documentElement.scrollHeight')
const { data } = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true, clip: { x: 0, y: 0, width: Number(w), height, scale: 1 } })
writeFileSync(out, Buffer.from(data, 'base64'))
console.log(JSON.stringify({ saved: out, height, ...overflow }))
ws.close()
browser.kill()
process.exit(0)
