import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  testIgnore: '**/generation.spec.ts',
  timeout: 30_000,
  workers: 1,
  use: {
    baseURL: 'http://127.0.0.1:4173',
    browserName: 'chromium',
    channel: 'msedge',
    viewport: { width: 1440, height: 960 },
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
  outputDir: '.artifacts/playwright',
  webServer: {
    command: 'npm run dev --workspace studio -- --host 127.0.0.1 --port 4173 --strictPort',
    url: 'http://127.0.0.1:4173',
    reuseExistingServer: !process.env.CI,
  },
});