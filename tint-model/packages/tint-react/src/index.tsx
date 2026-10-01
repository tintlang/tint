import { createElement, useEffect, useRef, type ComponentType, type CSSProperties } from "react";
import { createRoot } from "react-dom/client";
import {
  mount,
  type TintApp,
  type TintComponent,
  type TintEntry,
  type TintMountOptions,
} from "@tintlang/runtime";

export interface TintProps {
  source: string;
  entry?: TintEntry;
  className?: string;
  style?: CSSProperties;
  callbacks?: TintMountOptions["callbacks"];
  onError?: (error: Error) => void;
}

/** Mounts a Tint UI into a React-owned DOM island. */
export function Tint({
  source,
  entry = "App",
  className,
  style,
  callbacks,
  onError,
}: TintProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const appRef = useRef<TintApp | null>(null);
  const sourceRef = useRef(source);
  const errorRef = useRef(onError);

  sourceRef.current = source;
  errorRef.current = onError;

  useEffect(() => {
    let disposed = false;
    let mountedApp: TintApp | null = null;

    const start = async () => {
      if (!hostRef.current) return;
      const initialSource = sourceRef.current;
      try {
        mountedApp = await mount(initialSource, hostRef.current, { entry, callbacks });
        if (disposed) {
          mountedApp.unmount();
          return;
        }
        appRef.current = mountedApp;
        if (sourceRef.current !== initialSource) {
          const error = mountedApp.reload(sourceRef.current, { entry });
          if (error) errorRef.current?.(new Error(error));
        }
      } catch (error) {
        if (!disposed) errorRef.current?.(toError(error));
      }
    };

    void start();

    return () => {
      disposed = true;
      mountedApp?.unmount();
      if (appRef.current === mountedApp) appRef.current = null;
    };
  }, [entry]);

  useEffect(() => {
    const app = appRef.current;
    if (!app) return;
    const error = app.reload(source, { entry });
    if (error) errorRef.current?.(new Error(error));
  }, [source, entry]);

  return <div ref={hostRef} className={className} style={style} data-tint-react />;
}

function toError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}

/**
 * Turns a React component into a Tint host component:
 * `mount(src, el, { components: { Chart: reactComponent(Chart) } })` and, in
 * Tint, `Host { component||"Chart" props||{ ChartProps { ... } } }`.
 * Each props change re-renders the same React root.
 */
export function reactComponent<P extends object>(Component: ComponentType<P>): TintComponent {
  return (element, props) => {
    const root = createRoot(element);
    root.render(createElement(Component, props as P));
    return {
      update: (next) => root.render(createElement(Component, next as P)),
      destroy: () => root.unmount(),
    };
  };
}
