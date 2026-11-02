import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Tauri 期望一个固定端口的前端服务；打包产物在 dist/。
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**', '**/versions/**'] }
  },
  build: {
    target: 'chrome120',
    outDir: 'dist',
    emptyOutDir: true,
    sourcemap: false,
    // 关闭 JS/CSS 压缩：vite 6 锁定的 esbuild 0.25.12 在压缩本工程产物时会
    // 在 minifier 里段错误（`mangleIf` → `SimplifyUnusedExpr`，`unexpected fault address`），
    // 且与源码无关（同一份源码时崩时不崩）。桌面应用从本地盘加载资源，
    // 体积从 ~123KB 涨到 ~287KB 无实际影响，换来可复现的发布构建。
    minify: false
  }
});
