import type { ResponseInternal, RequestInternal } from "../core";
export declare function toInternalRequest(req: Request): Promise<RequestInternal | Error>;
export declare function toResponse(res: ResponseInternal): Response;
