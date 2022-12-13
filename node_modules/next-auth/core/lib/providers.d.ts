import type { InternalProvider } from "../types";
import type { Provider } from "../../providers";
/**
 * Adds `signinUrl` and `callbackUrl` to each provider
 * and deep merge user-defined options.
 */
export default function parseProviders(params: {
    providers: Provider[];
    url: URL;
    providerId?: string;
}): {
    providers: InternalProvider[];
    provider?: InternalProvider;
};
