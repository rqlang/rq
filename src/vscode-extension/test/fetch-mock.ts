export class FetchMock {
    private originalFetch: any;

    constructor() {
        this.originalFetch = global.fetch;
        global.fetch = jest.fn();
    }

    mockResponse(data: any, status: number = 200, ok: boolean = true) {
        (global.fetch as jest.Mock).mockResolvedValue({
            ok,
            status,
            json: async () => data,
            text: async () => JSON.stringify(data)
        });
    }

    mockError(error: Error) {
        (global.fetch as jest.Mock).mockRejectedValue(error);
    }

    get jestMock() {
        return global.fetch as jest.Mock;
    }

    restore() {
        global.fetch = this.originalFetch;
    }
}
