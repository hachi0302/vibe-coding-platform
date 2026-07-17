import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
import Icons from 'unplugin-icons/vite'

// Standalone Vitest config. We don't reuse vite.config.ts because that file
// exports an async factory wired for the Tauri dev server (fixed port 1420,
// vue-devtools, tailwind). Tests only need Vue SFC compilation and the
// `~icons/*` virtual-module resolver that components/icons.ts depends on.
export default defineConfig({
  plugins: [
    vue(),
    Icons({ compiler: 'vue3', scale: 1, defaultClass: 'iconify' }),
  ],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./test/setup.ts'],
    include: ['test/**/*.test.ts'],
    coverage: {
      provider: 'v8',
      reportsDirectory: './coverage',
      include: ['src/**/*.{ts,vue}'],
      // App.vue / views / modals are large stateful shells better covered by
      // e2e; the unit suite targets the pure logic + leaf components.
      exclude: [
        'src/**/*.d.ts',
        'src/main.ts',
        'src/vite-env.d.ts',
        'src/App.vue',
        'src/views/**',
        'src/modals/**',
        'src/components/topbar/**',
      ],
    },
  },
})
