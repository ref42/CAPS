import { execFileSync, spawn } from 'node:child_process';
import process from 'node:process';

const port = 5173;

function listeningPids() {
  try {
    const output = execFileSync('netstat', ['-ano'], { encoding: 'utf8' });
    const pids = new Set();
    for (const line of output.split(/\r?\n/)) {
      if (!line.includes(`:${port}`) || !line.includes('LISTENING')) continue;
      const parts = line.trim().split(/\s+/);
      const pid = Number(parts.at(-1));
      if (Number.isInteger(pid) && pid > 0 && pid !== process.pid) {
        pids.add(pid);
      }
    }
    return [...pids];
  } catch {
    return [];
  }
}

function pidExists(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

for (const pid of listeningPids()) {
  console.log(`Stopping stale process ${pid} on dev port ${port}`);
  try {
    process.kill(pid, 'SIGTERM');
  } catch {}
}

await new Promise((resolve) => setTimeout(resolve, 800));

for (const pid of listeningPids()) {
  if (!pidExists(pid)) continue;
  try {
    process.kill(pid, 'SIGKILL');
  } catch {}
}

await new Promise((resolve) => setTimeout(resolve, 300));

if (listeningPids().length) {
  console.error(`Port ${port} is still occupied. Close the stale process and rerun.`);
  process.exit(1);
}

const viteEntry = 'node_modules/vite/bin/vite.js';

const child = spawn(process.execPath, [viteEntry, '--host', '127.0.0.1', '--port', String(port), '--strictPort'], {
  stdio: 'inherit',
  shell: false,
});

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => {
    child.kill(signal);
    process.exit(0);
  });
}

child.on('exit', (code, signal) => {
  if (signal) process.kill(process.pid, signal);
  process.exit(code ?? 0);
});
