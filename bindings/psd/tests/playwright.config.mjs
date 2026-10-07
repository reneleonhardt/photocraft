export default {
    testDir: '.',
    testMatch: 'browser.mjs',
    projects: ['chromium', 'firefox', 'webkit'].map(browserName => ({ name: browserName, use: { browserName } })),
    workers: 1,
    timeout: 30_000,
    globalTimeout: 120_000,
    forbidOnly: true,
    reporter: 'line',
    outputDir: '../target/playwright',
};
