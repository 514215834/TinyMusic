import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [vue()],

  // CSS 压缩用 esbuild：vite8（rolldown）默认的 lightningcss 压缩器按其默认浏览器
  // 目标把标准 backdrop-filter 删成只剩 -webkit- 别名，打包版（WebView2）不认别名
  // 导致毛玻璃全部失效（dev 不走压缩所以看不出）；esbuild 只压缩不改写属性，
  // 且 css.lightningcss 选项在 vite8 中不会传给压缩器（targets 方案实测无效）
  build: {
    cssMinify: "esbuild",
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
