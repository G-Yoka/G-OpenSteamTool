import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    watch: {
      // 只忽略构建产物与编辑器的原子写入临时目录，避免 watcher 被临时文件锁死
      ignored: ["**/src-tauri/**", "**/*.tmpdir/**", "**/.*.tmp*"],
    },
  },
});
