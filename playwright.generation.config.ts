import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  testMatch: '**/generation.spec.ts',
  timeout: 30_000,
  workers: 1,
  use: {
    baseURL: 'http://127.0.0.1:4176',
    browserName: 'chromium',
    channel: 'msedge',
    viewport: { width: 1440, height: 960 },
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
  outputDir: '.artifacts/generation-playwright',
  webServer: {
    command: 'node tools/testing/studio-provider.mjs',
    url: 'http://127.0.0.1:4176',
    reuseExistingServer: false,
  },
});