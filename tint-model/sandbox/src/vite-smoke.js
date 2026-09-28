import source, { entry } from "./vite-smoke.tn";
import { mount } from "../../packages/tint-runtime/src/index.ts";

await mount(source, document.querySelector("#app"), { entry });
