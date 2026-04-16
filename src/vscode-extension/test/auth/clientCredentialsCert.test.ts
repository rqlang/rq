import * as crypto from 'crypto';
import { execFileSync } from 'child_process';
import { parsePemCert, createJwtAssertion, extractPemFromP12, base64url } from '../../src/rqClient';

jest.mock('child_process', () => ({ execFileSync: jest.fn() }));
const mockExecFileSync = execFileSync as jest.MockedFunction<typeof execFileSync>;

function makeTestPem(): { certDer: Buffer; privateKeyPem: Buffer; publicKey: crypto.KeyObject; combinedPem: Buffer } {
    const { privateKey, publicKey } = crypto.generateKeyPairSync('rsa', { modulusLength: 2048 });
    const privateKeyPem = Buffer.from(privateKey.export({ type: 'pkcs8', format: 'pem' }) as string);
    const certDer = Buffer.from('test-cert-der-content');
    const certPem = `-----BEGIN CERTIFICATE-----\n${certDer.toString('base64')}\n-----END CERTIFICATE-----`;
    const combinedPem = Buffer.from(`${certPem}\n${privateKeyPem.toString()}`);
    return { certDer, privateKeyPem, publicKey, combinedPem };
}

describe('parsePemCert', () => {
    let target: ReturnType<typeof makeTestPem>;

    beforeAll(() => { target = makeTestPem(); });

    it('extracts cert DER matching the CERTIFICATE block', () => {
        const result = parsePemCert(target.combinedPem);
        expect(result.certDer).toEqual(target.certDer);
    });

    it('extracts private key PEM containing a PRIVATE KEY block', () => {
        const result = parsePemCert(target.combinedPem);
        expect(result.privateKeyPem.toString()).toContain('PRIVATE KEY');
    });

    it('throws when CERTIFICATE block is missing', () => {
        expect(() => parsePemCert(target.privateKeyPem))
            .toThrow('No CERTIFICATE block found in PEM file');
    });

    it('throws when PRIVATE KEY block is missing', () => {
        const certOnly = Buffer.from(
            '-----BEGIN CERTIFICATE-----\nZmFrZQ==\n-----END CERTIFICATE-----',
        );
        expect(() => parsePemCert(certOnly))
            .toThrow('No PRIVATE KEY block found in PEM file');
    });

    it('handles RSA PRIVATE KEY (PKCS#1) format', () => {
        const { privateKey } = crypto.generateKeyPairSync('rsa', { modulusLength: 2048 });
        const pkcs1Pem = privateKey.export({ type: 'pkcs1', format: 'pem' }) as string;
        const certDer = Buffer.from('bytes');
        const certPem = `-----BEGIN CERTIFICATE-----\n${certDer.toString('base64')}\n-----END CERTIFICATE-----`;
        const pem = Buffer.from(`${certPem}\n${pkcs1Pem}`);
        const result = parsePemCert(pem);
        expect(result.privateKeyPem.toString()).toContain('RSA PRIVATE KEY');
    });
});

describe('createJwtAssertion', () => {
    let target: ReturnType<typeof makeTestPem>;

    beforeAll(() => { target = makeTestPem(); });

    it('returns a JWT with three dot-separated parts', () => {
        const jwt = createJwtAssertion(target.privateKeyPem, target.certDer, 'client', 'https://example.com/token');
        expect(jwt.split('.')).toHaveLength(3);
    });

    it('header contains alg RS256, typ JWT, and x5t derived from cert DER', () => {
        const jwt = createJwtAssertion(target.privateKeyPem, target.certDer, 'client', 'https://example.com/token');
        const header = JSON.parse(Buffer.from(jwt.split('.')[0], 'base64url').toString());
        const expectedX5t = base64url(crypto.createHash('sha1').update(target.certDer).digest());

        expect(header.alg).toBe('RS256');
        expect(header.typ).toBe('JWT');
        expect(header.x5t).toBe(expectedX5t);
    });

    it('payload contains iss, sub, aud matching client and token URL', () => {
        const clientId = 'my-client';
        const tokenUrl = 'https://idp.example.com/token';
        const jwt = createJwtAssertion(target.privateKeyPem, target.certDer, clientId, tokenUrl);
        const payload = JSON.parse(Buffer.from(jwt.split('.')[1], 'base64url').toString());

        expect(payload.iss).toBe(clientId);
        expect(payload.sub).toBe(clientId);
        expect(payload.aud).toBe(tokenUrl);
    });

    it('payload contains nbf 60s before iat and exp 300s after iat', () => {
        const jwt = createJwtAssertion(target.privateKeyPem, target.certDer, 'c', 'https://example.com/token');
        const payload = JSON.parse(Buffer.from(jwt.split('.')[1], 'base64url').toString());

        expect(payload.nbf).toBe(payload.iat - 60);
        expect(payload.exp).toBe(payload.iat + 300);
        expect(typeof payload.jti).toBe('string');
    });

    it('signature is verifiable with the corresponding RSA public key', () => {
        const jwt = createJwtAssertion(target.privateKeyPem, target.certDer, 'client', 'https://example.com/token');
        const parts = jwt.split('.');
        const signingInput = `${parts[0]}.${parts[1]}`;
        const signature = Buffer.from(parts[2], 'base64url');

        const verify = crypto.createVerify('RSA-SHA256');
        verify.update(signingInput);
        expect(verify.verify(target.publicKey, signature)).toBe(true);
    });
});

describe('extractPemFromP12', () => {
    const certPemOutput = Buffer.from('-----BEGIN CERTIFICATE-----\nZmFrZQ==\n-----END CERTIFICATE-----');
    const keyPemOutput = Buffer.from('-----BEGIN PRIVATE KEY-----\nZmFrZQ==\n-----END PRIVATE KEY-----');

    beforeEach(() => { mockExecFileSync.mockReset(); });

    it('calls openssl with correct args and returns parsed PEM', () => {
        mockExecFileSync
            .mockReturnValueOnce(certPemOutput)
            .mockReturnValueOnce(keyPemOutput);

        extractPemFromP12('/certs/client.p12', 'secret');

        expect(mockExecFileSync).toHaveBeenCalledWith(
            'openssl',
            expect.arrayContaining(['-in', '/certs/client.p12', '-passin', 'pass:secret', '-nokeys']),
        );
        expect(mockExecFileSync).toHaveBeenCalledWith(
            'openssl',
            expect.arrayContaining(['-in', '/certs/client.p12', '-passin', 'pass:secret', '-nocerts', '-nodes']),
        );
    });

    it('retries with -legacy flag when first attempt fails', () => {
        mockExecFileSync
            .mockImplementationOnce(() => { throw new Error('unsupported'); })
            .mockReturnValueOnce(certPemOutput)
            .mockReturnValueOnce(keyPemOutput);

        extractPemFromP12('/certs/client.p12', '');

        const lastCalls = mockExecFileSync.mock.calls.slice(1);
        expect(lastCalls.some(args => (args[1] as string[]).includes('-legacy'))).toBe(true);
    });

    it('throws a helpful error when both attempts fail', () => {
        mockExecFileSync.mockImplementation(() => { throw new Error('openssl not found'); });

        expect(() => extractPemFromP12('/certs/client.p12', 'pass'))
            .toThrow('Failed to parse .p12 certificate');
    });
});
