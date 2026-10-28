// CPython's normal command is python.exe on Windows and python3 on Fedora.
import { spawnSync } from 'node:child_process';

const command = process.platform === 'win32' ? 'python' : 'python3';
const result = spawnSync(command, process.argv.slice(2), { stdio: 'inherit' });
if (result.error) {
  console.error(`Cannot run ${command}; install Python 3.11 or newer and add it to PATH.`);
}
process.exit(result.status ?? 1);
