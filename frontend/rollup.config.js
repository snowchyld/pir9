import { visualizer } from "rollup-plugin-visualizer";

export default {
  plugins: [
    // Keep it last.
    visualizer(),
  ],
};
