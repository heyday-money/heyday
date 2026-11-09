import { defineConfig } from '@playwright/test'

// Pure financial tests need neither a browser nor a listening development server.
export default defineConfig({
  testDir: './tests',
  testMatch: ['domain-core.spec.ts', 'period.spec.ts', 'financial.spec.ts'],
})
