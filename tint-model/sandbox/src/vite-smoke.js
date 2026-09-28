import source, { entry } from "./vite-smoke.tn";
import { mount } from "../../packages/tint-runtime/src/index.ts";

await mount(source, document.querySelector("#app"), {
  entry,
  callbacks: {
    on_smoke_callback: ({ element }) => {
      element.setAttribute("data-callback-fired", "true");
    },
  },
});
