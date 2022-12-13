/// <reference types="react" />
import { Theme } from "../..";
export interface SignoutProps {
    url: URL;
    csrfToken: string;
    theme: Theme;
}
export default function SignoutPage(props: SignoutProps): JSX.Element;
