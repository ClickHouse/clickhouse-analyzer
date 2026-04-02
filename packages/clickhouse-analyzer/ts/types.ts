import type { SyntaxKind } from "./syntax-kind.js";

/** Raw token from the WASM parser (JSON-serialized). */
export interface RawToken {
    kind: SyntaxKind;
    start: number;
    end: number;
}

/** Raw tree node from the WASM parser (JSON-serialized). */
export interface RawTree {
    kind: SyntaxKind;
    start: number;
    end: number;
    children: RawChild[];
}

/** A child is either a token or a subtree. */
export type RawChild =
    | { Token: RawToken }
    | { Tree: RawTree };

/** Raw parse error from the WASM parser. */
export interface RawSyntaxError {
    message: string;
    range: [number, number];
}

/** Raw parse result from the WASM parser. */
export interface RawParseResult {
    tree: RawTree;
    errors: RawSyntaxError[];
    source: string;
}
