// Auto-managed by `zero dev` and `zero init`. Editing this file by hand
// will lose changes the next time the CLI runs.

declare module "zero/http" {
  export class HttpError extends Error {
    readonly status: number;
    readonly statusText: string;
    readonly body: unknown;
    constructor(status: number, statusText: string, body: unknown);
  }

  export interface HttpInit extends RequestInit {
    /** Override the constructor-time `fetch` for this call only. */
    fetch?: typeof fetch;
  }

  export type Middleware = (
    req: Request,
    next: (req: Request) => Promise<Response>,
  ) => Promise<Response>;

  export interface HttpClient {
    use(mw: Middleware): HttpClient;
    get<T = unknown>(url: string, init?: HttpInit): Promise<T>;
    post<T = unknown>(url: string, body?: unknown, init?: HttpInit): Promise<T>;
    put<T = unknown>(url: string, body?: unknown, init?: HttpInit): Promise<T>;
    patch<T = unknown>(url: string, body?: unknown, init?: HttpInit): Promise<T>;
    delete<T = unknown>(url: string, init?: HttpInit): Promise<T>;
    request<T = unknown>(
      input: Request | URL | string,
      init?: HttpInit,
    ): Promise<T>;
  }

  export function createHttp(opts?: { fetch?: typeof fetch }): HttpClient;
}
