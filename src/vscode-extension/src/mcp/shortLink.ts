import * as crypto from 'crypto';
import * as http from 'http';
import { AddressInfo } from 'net';

export interface ShortLink {
    url: string;
    close(): void;
}

const LOOPBACK_HOST = '127.0.0.1';

export function openShortLink(target: string, timeoutMs: number): Promise<ShortLink> {
    const id = crypto.randomBytes(16).toString('hex');
    let host = '';
    const server = http.createServer((req, res) => {
        if (req.method !== 'GET' || req.url !== `/${id}` || req.headers.host !== host) {
            res.writeHead(404, { 'Connection': 'close' });
            res.end();
            return;
        }
        res.writeHead(302, { 'Location': target, 'Cache-Control': 'no-store', 'Referrer-Policy': 'no-referrer', 'Connection': 'close' });
        res.end();
    });
    server.unref();
    const close = () => {
        clearTimeout(timer);
        server.close();
        server.closeAllConnections();
    };
    const timer = setTimeout(close, timeoutMs);
    timer.unref();
    return new Promise((resolve, reject) => {
        server.once('error', err => {
            close();
            reject(err);
        });
        server.listen(0, LOOPBACK_HOST, () => {
            host = `${LOOPBACK_HOST}:${(server.address() as AddressInfo).port}`;
            resolve({ url: `http://${host}/${id}`, close });
        });
    });
}
