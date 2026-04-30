#!/usr/bin/env node
// Cross-platform port cleaner used before `vite dev` to recover from
// orphan dev processes that survived a killed `pnpm tauri dev` run.
// See docs/status/2026-04-30-ide-e2e-report.md §H1.

import { createServer } from 'node:net';
import { execFileSync } from 'node:child_process';
import { exit, platform } from 'node:process';

const port = Number(process.argv[2]);
if (!Number.isInteger(port) || port < 1 || port > 65535) {
  console.error(`clean-port: invalid port "${process.argv[2]}"`);
  exit(2);
}

function probeBind(p, host) {
  return new Promise((resolve) => {
    const srv = createServer();
    srv.once('error', (err) => {
      // Address family unavailable (e.g., IPv6 disabled) cannot be holding the
      // port — only EADDRINUSE means held.
      const free = err.code === 'EADDRNOTAVAIL' || err.code === 'EAFNOSUPPORT';
      resolve(free);
    });
    srv.once('listening', () => srv.close(() => resolve(true)));
    srv.listen(p, host);
  });
}

async function isPortFree(p) {
  // Vite resolves `localhost` to `::1` on Windows + Node >= 17, so probe both
  // loopbacks; the port is free only when neither family is held.
  const [v4, v6] = await Promise.all([probeBind(p, '127.0.0.1'), probeBind(p, '::1')]);
  return v4 && v6;
}

function findPidsWindows(p) {
  // No `-p TCP` filter: Windows lists IPv6 TCP listeners under the `TCPv6`
  // proto, which `-p TCP` excludes. UDP rows lack `LISTENING` so the
  // state-filter below skips them.
  let out;
  try {
    out = execFileSync('netstat', ['-ano'], { encoding: 'utf8' });
  } catch {
    return [];
  }
  const pids = new Set();
  const needle = `:${p}`;
  for (const line of out.split(/\r?\n/)) {
    if (!line.includes('LISTENING')) continue;
    const cols = line.trim().split(/\s+/);
    if (cols.length < 5) continue;
    const local = cols[1];
    if (!local.endsWith(needle)) continue;
    const pid = Number(cols[4]);
    if (Number.isInteger(pid) && pid > 0) pids.add(pid);
  }
  return [...pids];
}

function findPidsPosix(p) {
  try {
    const out = execFileSync('lsof', ['-tiTCP:' + p, '-sTCP:LISTEN'], { encoding: 'utf8' });
    return out.split(/\r?\n/).map((s) => Number(s.trim())).filter((n) => Number.isInteger(n) && n > 0);
  } catch {
    return [];
  }
}

function killPid(pid) {
  if (platform === 'win32') {
    execFileSync('taskkill', ['/F', '/PID', String(pid)], { stdio: 'ignore' });
  } else {
    process.kill(pid, 'SIGKILL');
  }
}

const free = await isPortFree(port);
if (free) {
  console.log(`clean-port: ${port} is free`);
  exit(0);
}

const pids = platform === 'win32' ? findPidsWindows(port) : findPidsPosix(port);
if (pids.length === 0) {
  console.error(`clean-port: ${port} is held but no PID resolvable; aborting`);
  exit(1);
}

for (const pid of pids) {
  try {
    killPid(pid);
    console.log(`clean-port: killed PID ${pid} on port ${port}`);
  } catch (err) {
    console.error(`clean-port: failed to kill PID ${pid}: ${err.message}`);
    exit(1);
  }
}

const stillFree = await isPortFree(port);
if (!stillFree) {
  console.error(`clean-port: ${port} still held after kill; aborting`);
  exit(1);
}
console.log(`clean-port: ${port} reclaimed`);
