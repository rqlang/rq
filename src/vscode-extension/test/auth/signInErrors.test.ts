import { callbackListenError, providerRefusedError, tokenExchangeError, verifyState } from '../../src/auth/signInErrors';

describe('verifyState', () => {
    it('accepts the state it sent', () => {
        expect(() => verifyState('abc', 'abc')).not.toThrow();
    });

    it('explains that an address without state was most likely opened from an incomplete link', () => {
        expect(() => verifyState('abc', null)).toThrow('most likely opened from an incomplete link, cut off before its state');
    });

    it('explains that an address with another state comes from another sign-in', () => {
        expect(() => verifyState('abc', 'xyz')).toThrow('comes from another sign-in');
    });
});

describe('providerRefusedError', () => {
    it('quotes the error and its description', () => {
        expect(providerRefusedError('access_denied', 'User said no').message).toBe('The authorization server refused the sign-in: access_denied (User said no).');
    });

    it('leaves out a missing description', () => {
        expect(providerRefusedError('access_denied', null).message).toBe('The authorization server refused the sign-in: access_denied.');
    });
});

describe('tokenExchangeError', () => {
    it('names the token url, the status and what the server said', () => {
        expect(tokenExchangeError('https://auth.example.com/token', 400, 'Code not valid').message)
            .toContain('rejected the authorization code at https://auth.example.com/token (HTTP 400: Code not valid)');
    });
});

describe('callbackListenError', () => {
    it('explains a port in use', () => {
        const error = Object.assign(new Error('listen EADDRINUSE'), { code: 'EADDRINUSE' });
        expect(callbackListenError('http://localhost:3000', 3000, error).message).toContain('Port 3000 of the redirect_uri http://localhost:3000 is in use by another program');
    });

    it('passes on any other listen failure', () => {
        const error = Object.assign(new Error('listen EACCES'), { code: 'EACCES' });
        expect(callbackListenError('http://localhost:80', 80, error).message).toBe('rq cannot listen on http://localhost:80 to receive the sign-in: listen EACCES.');
    });
});
