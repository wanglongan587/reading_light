import { build } from 'esbuild';
import { mkdir, copyFile } from 'node:fs/promises';
await mkdir('dist', { recursive: true });
await build({ entryPoints: ['src/reader.ts'], bundle: true, outfile: 'dist/reader.js', format: 'iife', target: 'es2020', minify: true, legalComments: 'linked' });
await copyFile('src/index.html', 'dist/index.html');
await copyFile('src/reader.css', 'dist/reader.css');
