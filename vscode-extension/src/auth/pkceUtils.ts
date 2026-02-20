import crypto from 'crypto';

export function generateCodeVerifier(): string {
    return crypto.randomBytes(32).toString('base64url');
}

export function generateCodeChallenge(verifier: string, method: 'S256' | 'plain'): string {
    if (method === 'plain') {
        return verifier;
    }
    return crypto.createHash('sha256')
        .update(verifier)
        .digest('base64url');
}
