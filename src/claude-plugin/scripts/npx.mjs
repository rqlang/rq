import { spawn } from 'node:child_process';

const child = spawn('npx', process.argv.slice(2), { stdio: 'inherit', shell: process.platform === 'win32' });

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
