import { defineConfig, type RolldownPlugin } from "rolldown";
import { visualizer } from "rollup-plugin-visualizer";

export default defineConfig({
  plugins: [visualizer() as RolldownPlugin],
});
