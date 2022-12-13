/// <reference types="react" />
import { Theme } from "../..";
/**
 * The following errors are passed as error query parameters to the default or overridden error page.
 *
 * [Documentation](https://next-auth.js.org/configuration/pages#error-page) */
export declare type ErrorType = "default" | "configuration" | "accessdenied" | "verification";
export interface ErrorProps {
    url?: URL;
    theme?: Theme;
    error?: ErrorType;
}
/** Renders an error page. */
export default function ErrorPage(props: ErrorProps): {
    status: any;
    html: JSX.Element;
};
