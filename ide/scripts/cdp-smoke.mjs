#!/usr/bin/env node
// CDP smoke test for the running AIL Tauri WebView2 (port 9223).
// Proves we can attach, read DOM state, and screenshot — no tauri-driver, no WDIO.

import { setTimeout as delay } from 'node:timers/promises';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const PORT = process.env.CDP_PORT ?? '9223';
const ENDPOINT = `http://localhost:${PORT}`;
const SCREENSHOT_PATH = resolve(import.meta.dirname, '..', 'e2e', 'screenshots', 'cdp-smoke.png');

async function listTargets() {
  const res = await fetch(`${ENDPOINT}/json/list`);
  if (!res.ok) throw new Error(`/json/list ${res.status}`);
  return res.json();
}

function pickAilTarget(targets) {
  // The Tauri WebView2 page loads from http://localhost:1420 in dev mode,
  // or tauri://localhost in release. Pick the page-type target whose URL
  // matches one of those, falling back to anything titled "AIL IDE".
  return targets.find(t =>
    t.type === 'page' &&
    (t.url?.startsWith('http://localhost:1420') ||
     t.url?.startsWith('tauri://localhost') ||
     t.title === 'AIL IDE')
  );
}

class CDPClient {
  constructor(wsUrl) {
    this.ws = new WebSocket(wsUrl);
    this.id = 0;
    this.pending = new Map();
    this.ready = new Promise((res, rej) => {
      this.ws.onopen = () => res();
      this.ws.onerror = (e) => rej(new Error(`WS error: ${e.message ?? e.type}`));
    });
    this.ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id != null && this.pending.has(msg.id)) {
        const { resolve, reject } = this.pending.get(msg.id);
        this.pending.delete(msg.id);
        if (msg.error) reject(new Error(`${msg.error.code}: ${msg.error.message}`));
        else resolve(msg.result);
      }
    };
  }
  async send(method, params = {}) {
    await this.ready;
    const id = ++this.id;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method, params }));
    });
  }
  close() { this.ws.close(); }
}

async function main() {
  console.log(`[cdp-smoke] connecting to ${ENDPOINT}`);
  const targets = await listTargets();
  console.log(`[cdp-smoke] ${targets.length} targets:`);
  for (const t of targets) {
    console.log(`  - ${t.type} ${JSON.stringify(t.title)} ${t.url}`);
  }
  const target = pickAilTarget(targets);
  if (!target) {
    console.error('[cdp-smoke] no AIL Tauri page target found');
    process.exit(1);
  }
  console.log(`[cdp-smoke] target: ${target.title} (${target.url})`);

  const client = new CDPClient(target.webSocketDebuggerUrl);
  await client.send('Page.enable');
  await client.send('Runtime.enable');

  // 1. Read document.title and basic DOM snapshot.
  const titleRes = await client.send('Runtime.evaluate', {
    expression: 'document.title',
    returnByValue: true,
  });
  console.log(`[cdp-smoke] document.title = ${JSON.stringify(titleRes.result.value)}`);

  // 2. Read Tauri runtime markers — proves we're inside the real WebView2.
  const tauriRes = await client.send('Runtime.evaluate', {
    expression: 'JSON.stringify({ isTauri: typeof window.isTauri, hasInternals: typeof window.__TAURI_INTERNALS__ })',
    returnByValue: true,
  });
  console.log(`[cdp-smoke] tauri markers = ${tauriRes.result.value}`);

  // 3. Count visible regions (TitleBar, Outline, Stage, RightSidebar).
  const regionsRes = await client.send('Runtime.evaluate', {
    expression: `JSON.stringify(['region-titlebar','region-outline','region-stage','right-sidebar']
      .map(id => ({ id, present: !!document.querySelector('[data-testid="' + id + '"]') })))`,
    returnByValue: true,
  });
  console.log(`[cdp-smoke] regions = ${regionsRes.result.value}`);

  // 4. Screenshot proof.
  const shot = await client.send('Page.captureScreenshot', { format: 'png' });
  const buf = Buffer.from(shot.data, 'base64');
  // Ensure directory exists.
  const { mkdirSync } = await import('node:fs');
  mkdirSync(resolve(SCREENSHOT_PATH, '..'), { recursive: true });
  writeFileSync(SCREENSHOT_PATH, buf);
  console.log(`[cdp-smoke] screenshot saved -> ${SCREENSHOT_PATH} (${buf.length} bytes)`);

  client.close();
  await delay(100);
  console.log('[cdp-smoke] OK');
}

main().catch((err) => {
  console.error('[cdp-smoke] FAILED:', err.message);
  process.exit(1);
});
