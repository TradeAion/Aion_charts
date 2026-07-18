import { defineConfig } from "@playwright/test";

const port = Number.parseInt(process.env.AION_TEST_PORT ?? "4174", 10);

export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  workers: 1,
  reporter: [["list"], ["html", { open: "never" }]],
  outputDir: "test-results",
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    channel: "chromium",
    viewport: { width: 1280, height: 720 },
    deviceScaleFactor: 1.5,
    colorScheme: "light",
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
    launchOptions: {
      args: [
        "--enable-unsafe-webgpu",
        "--use-webgpu-adapter=swiftshader",
        "--enable-dawn-features=allow_unsafe_apis",
        "--disable-dawn-features=use_dxc",
        "--enable-webgpu-developer-features",
        "--use-gpu-in-tests",
        "--enable-accelerated-2d-canvas",
      ],
    },
  },
  webServer: {
    command: "node test_server.mjs",
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});
