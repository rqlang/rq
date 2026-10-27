import { spawn } from 'node:child_process';

const args = process.argv.slice(2);
const child = process.platform === 'win32'
    ? spawn(['npx', ...args].join(' '), { stdio: 'inherit', shell: true })
    : spawn('npx', args, { stdio: 'inherit' });

for (const signal of ['SIGINT', 'SIGTERM']) {
    process.on(signal, () => child.kill(signal));
}

child.on('error', error => {
    console.error(`Could not start npx: ${error.message}`);
    process.exitCode = 1;
});

child.on('exit', (code, signal) => {
    process.exitCode = code ?? (signal ? 1 : 0);
});
