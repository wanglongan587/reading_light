import { defineConfig } from '@playwright/test';
export default defineConfig({ testDir:'tests',timeout:30000,fullyParallel:false,workers:1,
 use:{channel:process.env.PLAYWRIGHT_CHANNEL || undefined,baseURL:'http://127.0.0.1:4178',viewport:{width:900,height:680},screenshot:'only-on-failure'},
 webServer:{command:'node test-server.mjs',url:'http://127.0.0.1:4178/index.html',reuseExistingServer:false},
 reporter:'list' });
