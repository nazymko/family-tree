// Started by scripts/lib/tasks.mjs. Reads "add <pid>" / "del <pid>" lines on stdin and, when stdin closes (the script
// that started this died, even if it was killed hard and could run no cleanup), kills every pid still listed together
// with its children. Only pids the script itself started are ever listed; nothing is killed by name.
import { spawnSync } from 'node:child_process';

const pids = new Set();
let buffer = '';

function cleanup() {
  for (const pid of pids) {
    if (process.platform === 'win32') spawnSync('taskkill', ['/PID', String(pid), '/T', '/F'], { stdio: 'ignore' });
    else {
      try {
        process.kill(-pid, 'SIGKILL');
      } catch {
        /* gone already */
      }
    }
  }
  process.exit(0);
}

process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => {
  buffer += chunk;
  let i;
  while ((i = buffer.indexOf('\n')) >= 0) {
    const [verb, pid] = buffer.slice(0, i).trim().split(' ');
    buffer = buffer.slice(i + 1);
    if (verb === 'add') pids.add(Number(pid));
    else if (verb === 'del') pids.delete(Number(pid));
    else if (verb === 'done') process.exit(0);
  }
});
process.stdin.on('end', cleanup);
process.stdin.on('close', cleanup);
process.stdin.on('error', cleanup);
