import * as http from 'http';
import { openShortLink, ShortLink } from '../../src/mcp/shortLink';

const TARGET = 'https://auth.example.com/authorize?client_id=app&code_challenge=abc&state=xyz';

function get(url: string, headers: Record<string, string> = {}): Promise<{ status: number; location?: string }> {
    return new Promise((resolve, reject) => {
        http.get(url, { headers }, res => {
            res.resume();
            res.on('end', () => resolve({ status: res.statusCode ?? 0, location: res.headers.location }));
        }).on('error', reject);
    });
}

describe('openShortLink', () => {
    let target: ShortLink | undefined;

    afterEach(() => {
        target?.close();
        target = undefined;
    });

    it('serves a short loopback url', async () => {
        target = await openShortLink(TARGET, 60_000);
        expect(target.url).toMatch(/^http:\/\/127\.0\.0\.1:\d+\/[0-9a-f]{32}$/);
    });

    it('redirects to the whole target url', async () => {
        target = await openShortLink(TARGET, 60_000);
        expect(await get(target.url)).toEqual({ status: 302, location: TARGET });
    });

    it('redirects every time it is opened while it is open', async () => {
        target = await openShortLink(TARGET, 60_000);
        await get(target.url);
        expect((await get(target.url)).location).toBe(TARGET);
    });

    it('does not reveal the target on another path', async () => {
        target = await openShortLink(TARGET, 60_000);
        expect(await get(`${new URL(target.url).origin}/other`)).toEqual({ status: 404, location: undefined });
    });

    it('does not reveal the target to a request for another host', async () => {
        target = await openShortLink(TARGET, 60_000);
        expect(await get(target.url, { host: 'attacker.example.com' })).toEqual({ status: 404, location: undefined });
    });

    it('stops answering once closed', async () => {
        target = await openShortLink(TARGET, 60_000);
        target.close();
        await expect(get(target.url)).rejects.toThrow();
    });

    it('stops answering once it times out', async () => {
        target = await openShortLink(TARGET, 10);
        await new Promise(resolve => setTimeout(resolve, 50));
        await expect(get(target.url)).rejects.toThrow();
    });
});
