import { defineConfig, loadEnv } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import Icons from "unplugin-icons/vite";
import vueDevTools from "vite-plugin-vue-devtools";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async ({ command, mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  // 优先读 .env*，其次读 shell 环境变量；用于 Vue DevTools 的 "Open in editor"
  // 跳转目标编辑器（如 code / cursor / webstorm）。未设置则走 launch-editor 默认探测。
  const devtoolsEditor = env.LAUNCH_EDITOR || process.env.LAUNCH_EDITOR;

  return {
    plugins: [
      vue(),
      tailwindcss(),
      // Iconify 编译期出 Vue 组件，运行时不联网（Tauri 离线友好）。
      // 用法：import IconFoo from '~icons/lucide/foo-name'
      Icons({ compiler: "vue3", scale: 1, defaultClass: "iconify" }),
      // Vue DevTools 仅在 dev 启用，避免污染生产打包
      command === "serve"
        ? devtoolsEditor
          ? vueDevTools({ launchEditor: devtoolsEditor })
          : vueDevTools()
        : null,
    ],

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
      // 3. tell Vite to ignore watching `src-tauri`, and any git worktrees created
      //    under the project. Worktrees hold full checkouts (incl. copies of `src/`),
      //    so without this a `git worktree add/remove` fires a flood of add/unlink
      //    events → Vite full-reloads → the window flashes white mid-operation.
      ignored: [
        "**/src-tauri/**",
        "**/.claude/worktrees/**",
        "**/.claude-worktrees/**",
      ],
    },
  },
  };
});
