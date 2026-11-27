// Drives the Tessera development window's page through the WebView2 DevTools
// protocol, for walking through a flow and taking screenshots. Only the
// app's own page is touched, never the rest of the screen.
//
// Start the dev app with
//   $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9333 --remote-allow-origins=*'
// then run steps in order, for example:
//   node tools/cdp-run.mjs "click:Search" "type:Search by name=setup" "wait:30 matches" "shot:target/a.png"
//
// Steps:
//   wait:TEXT        until the page text contains TEXT (15 s)
//   gone:TEXT        until the page text no longer contains TEXT (60 s)
//   click:TEXT       click the button, link, label or row whose text or aria-label is TEXT
//   clickre:REGEX    the same, matching a regular expression
//   type:LABEL=VALUE set the input whose aria-label is LABEL
//   select:LABEL=VALUE choose VALUE in the select whose aria-label is LABEL
//   shot:FILE        screenshot of the page
//   text:FILE        the page text, to a file
//   sleep:MS
//   eval:JS          run JavaScript in the page and print the result
//   net:on           start recording requests to the app's backend
//   net:FILE         write the recorded requests to FILE
import { writeFileSync } from 'node:fs'

const steps = process.argv.slice(2)
const pages = await (await fetch('http://127.0.0.1:9333/json')).json()
// The dev server page, or a release build's page (tauri.localhost).
const page = pages.find((p) => p.type === 'page' && /^http:\/\/(localhost:5173|tauri\.localhost)/.test(p.url))
if (!page) throw new Error('Tessera page not found on port 9333')
const ws = new WebSocket(page.webSocketDebuggerUrl)
let next = 1
const pending = new Map()
// Requests the page sends to the app's backend (Tauri's ipc:// protocol),
// recorded between "net:on" and "net:FILE".
const net = []
const t0 = Date.now()
ws.onmessage = (e) => {
  const msg = JSON.parse(e.data)
  if (msg.id && pending.has(msg.id)) {
    pending.get(msg.id)(msg)
    pending.delete(msg.id)
  } else if (msg.method === 'Network.requestWillBeSent') {
    const url = msg.params.request.url
    if (/ipc/.test(url)) net.push({ at: Date.now() - t0, url: url.replace(/^.*?\/\/[^/]+\//, ''), body: msg.params.request.postData?.slice(0, 120) })
  }
}
const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const id = next++
    pending.set(id, (m) => (m.error ? reject(new Error(m.error.message)) : resolve(m.result)))
    ws.send(JSON.stringify({ id, method, params }))
  })
await new Promise((r) => (ws.onopen = r))
const evaluate = async (expression) => {
  const r = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? 'page error')
  return r.result.value
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
const until = async (fn, ms, what) => {
  const end = Date.now() + ms
  while (Date.now() < end) {
    if (await fn()) return
    await sleep(150)
  }
  throw new Error(`timed out: ${what}`)
}
// Finds a clickable element by exact text / aria-label (or a regex) and clicks it.
const clickJs = (matcher) => `(() => {
  const m = ${matcher};
  const els = [...document.querySelectorAll('button, a, label, [role=option], [role=tab], input[type=checkbox], summary')];
  const text = (el) => (el.getAttribute('aria-label') || el.textContent || '').replace(/\\s+/g, ' ').trim();
  const el = els.find((el) => !el.disabled && m(text(el)));
  if (!el) return false;
  el.scrollIntoView({ block: 'center' });
  el.click();
  return true;
})()`

for (const step of steps) {
  const i = step.indexOf(':')
  const kind = step.slice(0, i)
  const arg = step.slice(i + 1)
  if (kind === 'wait') {
    await until(() => evaluate(`document.body.innerText.includes(${JSON.stringify(arg)})`), 15000, `text "${arg}"`)
  } else if (kind === 'gone') {
    await until(() => evaluate(`!document.body.innerText.includes(${JSON.stringify(arg)})`), 60000, `text "${arg}" to go`)
  } else if (kind === 'click' || kind === 'clickre') {
    const matcher = kind === 'click' ? `(t) => t === ${JSON.stringify(arg)}` : `(t) => new RegExp(${JSON.stringify(arg)}).test(t)`
    await until(() => evaluate(clickJs(matcher)), 10000, `something to click: ${arg}`)
    await sleep(250)
  } else if (kind === 'type' || kind === 'select') {
    const eq = arg.indexOf('=')
    const label = arg.slice(0, eq)
    const value = arg.slice(eq + 1)
    const tag = kind === 'type' ? 'input' : 'select'
    const proto = kind === 'type' ? 'HTMLInputElement' : 'HTMLSelectElement'
    await until(
      () =>
        evaluate(`(() => {
          const el = document.querySelector('${tag}[aria-label=' + JSON.stringify(${JSON.stringify(label)}) + ']');
          if (!el) return false;
          Object.getOwnPropertyDescriptor(${proto}.prototype, 'value').set.call(el, ${JSON.stringify(value)});
          el.dispatchEvent(new Event('${kind === 'type' ? 'input' : 'change'}', { bubbles: true }));
          return true;
        })()`),
      10000,
      `${tag} "${label}"`,
    )
    await sleep(400)
  } else if (kind === 'shot') {
    await sleep(300)
    const { data } = await send('Page.captureScreenshot', { format: 'png' })
    writeFileSync(arg, Buffer.from(data, 'base64'))
    console.log(`saved ${arg}`)
  } else if (kind === 'text') {
    writeFileSync(arg, await evaluate('document.body.innerText'))
    console.log(`saved ${arg}`)
  } else if (kind === 'sleep') {
    await sleep(Number(arg))
  } else if (kind === 'net') {
    if (arg === 'on') await send('Network.enable')
    else {
      writeFileSync(arg, net.map((n) => `${n.at}ms ${n.url} ${n.body ?? ''}`).join('\n'))
      console.log(`saved ${arg} (${net.length} requests)`)
    }
  } else if (kind === 'eval') {
    console.log(JSON.stringify(await evaluate(arg)))
  } else {
    throw new Error(`unknown step ${step}`)
  }
  console.log(`ok ${step}`)
}
ws.close()
