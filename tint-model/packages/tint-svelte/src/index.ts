import {
  mount,
  type TintApp,
  type TintEntry,
  type TintMountOptions,
} from "@tintlang/runtime";

export interface TintActionParams {
  source: string;
  entry?: TintEntry;
  callbacks?: TintMountOptions["callbacks"];
  onError?: (error: Error) => void;
}

/** Mounts Tint into a Svelte-owned element with use:tint. */
export function tint(node: HTMLElement, params: TintActionParams) {
  let current = params;
  let app: TintApp | null = null;
  let disposed = false;
  let generation = 0;

  const start = async () => {
    const currentGeneration = ++generation;
    try {
      const nextApp = await mount(current.source, node, {
        entry: current.entry,
        callbacks: current.callbacks,
      });
      if (disposed || currentGeneration !== generation) {
        nextApp.unmount();
        return;
      }
      app = nextApp;
    } catch (error) {
      if (!disposed) current.onError?.(toError(error));
    }
  };

  void start();

  return {
    update(next: TintActionParams) {
      const previous = current;
      current = next;

      if (next.entry !== previous.entry) {
        app?.unmount();
        app = null;
        void start();
        return;
      }

      if (!app) {
        generation += 1;
        void start();
        return;
      }
      const error = app.reload(next.source, { entry: next.entry });
      if (error) next.onError?.(new Error(error));
    },
    destroy() {
      disposed = true;
      generation += 1;
      app?.unmount();
      app = null;
    },
  };
}

function toError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}
