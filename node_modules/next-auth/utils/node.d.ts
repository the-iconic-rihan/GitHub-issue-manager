/// <reference types="node" />
import type { IncomingMessage, ServerResponse } from "http";
import type { GetServerSidePropsContext, NextApiRequest } from "next";
export declare function setCookie(res: any, value: string): void;
export declare function getBody(req: IncomingMessage | NextApiRequest | GetServerSidePropsContext["req"]): {
    body: ReadableStream<any>;
} | {
    body: string;
} | undefined;
/**
 * Extract the full request URL from the environment.
 * NOTE: It does not verify if the host should be trusted.
 */
export declare function getURL(url: string | undefined, headers: Headers): URL | Error;
export declare function setHeaders(headers: Headers, res: ServerResponse): void;
declare global {
    namespace NodeJS {
        interface ProcessEnv {
            AUTH_TRUST_HOST?: string;
            NEXTAUTH_URL?: string;
            NEXTAUTH_SECRET?: string;
            VERCEL?: "1";
        }
    }
}
