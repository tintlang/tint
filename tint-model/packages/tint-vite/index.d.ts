import type { Plugin } from "vite";

export interface TintViteOptions {
  /** Entry ui function used when a module does not provide one explicitly. */
  entry?: string;
}

export default function tint(options?: TintViteOptions): Plugin;
