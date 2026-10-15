import { defineConfig } from '@playwright/test';

const port = Number(process.env.AISLIDE_TEST_PORT ?? '4173');
if (!Number.isInteger(port) || port < 1024 || port > 65535) throw new Error('AISLIDE_TEST_PORT must be an integer from 1024 to 65535');
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: './tests/e2e',
  testIgnore: '**/generation.spec.ts',
  timeout: 30_000,
  workers: 1,
  use: {
    baseURL,
    browserName: 'chromium',
    channel: 'msedge',
    viewport: { width: 1440, height: 960 },
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
  outputDir: '.artifacts/playwright',
  webServer: {
    command: `npm run dev --workspace studio -- --host 127.0.0.1 --port ${port} --strictPort`,
    url: baseURL,
    reuseExistingServer: !process.env.CI,
  },
});