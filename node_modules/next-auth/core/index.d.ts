import type { AuthOptions } from "./types";
/**
 * The core functionality of `next-auth`.
 * It receives a standard [`Request`](https://developer.mozilla.org/en-US/docs/Web/API/Request)
 * and returns a standard [`Response`](https://developer.mozilla.org/en-US/docs/Web/API/Response).
 */
export declare function AuthHandler(request: Request, options: AuthOptions): Promise<Response>;
