import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  testMatch: 'office-runtime.spec.ts',
  timeout: 60_000,
  workers: 1,
  outputDir: '.artifacts/playwright-office',
  use: { baseURL: 'http://127.0.0.1:4198', browserName: 'chromium', channel: 'msedge', viewport: { width: 1280, height: 800 }, screenshot: 'only-on-failure', trace: 'retain-on-failure' },
  webServer: {
    command: 'node tools/office-content-server.mjs --http-preview --preview .artifacts\\office-interactive\\dashboard.json',
    url: 'http://127.0.0.1:4198/preview.html',
    reuseExistingServer: false,
  },
});
