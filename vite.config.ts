import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true, watch: { ignored: ['**/src-tauri/**', '**/output/**'] } },
  build: { target: 'es2022', sourcemap: false },
  test: { include: ['tests/frontend/**/*.test.ts'], environment: 'node' },
});
