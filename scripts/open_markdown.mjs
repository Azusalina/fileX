#!/usr/bin/env node
// Reuse toolkit's renderer without changing its files or occupying its normal port.
import { parseArgs } from 'node:util';
import { resolve, dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
// The TUI closes startup pipes once it has the URL.
for (const stream of [process.stdout, process.stderr]) stream.on('error', error => { if (error.code !== 'EPIPE') process.exitCode = 1; });

let app;
let timer;
let closing = false;
async function close() {
  if (closing) return;
  closing = true;
  clearTimeout(timer);
  await app?.close();
}
try {
  const { values, positionals } = parseArgs({
    options: { toolkit: { type: 'string' }, browser: { type: 'string', default: 'firefox' },
      'idle-seconds': { type: 'string', default: '1800' } }, allowPositionals: true,
  });
  if (positionals.length !== 1) throw new Error('需要一个 Markdown 文件路径');
  const idle = Number(values['idle-seconds']);
  if (!Number.isFinite(idle) || idle < 1) throw new Error('idle-seconds 必须大于等于 1');
  const toolkit = resolve(values.toolkit ?? resolve(dirname(fileURLToPath(import.meta.url)), '../../toolkit'));
  const { createToolkit } = await import(pathToFileURL(resolve(toolkit, 'src/server.js')));
  app = await createToolkit({ port: 0, filename: resolve(positionals[0]), baseDir: toolkit });
  // Each preview has its own initial file; existing toolkit pages remain independent.
  let viewers = 0;
  function schedule() {
    clearTimeout(timer);
    if (closing) return;
    if (!viewers) timer = setTimeout(() => close().catch(console.error), idle * 1000);
  }
  app.server.on('request', (request, response) => {
    if (request.url?.split('?')[0] === '/api/events') {
      viewers++;
      clearTimeout(timer);
      response.on('close', () => { viewers--; schedule(); });
    } else schedule();
  });
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => close().catch(console.error));
  const browser = spawn(values.browser, ['--new-tab', app.url], { stdio: 'ignore' });
  browser.unref();
  browser.on('error', error => { console.error(error.message); process.exitCode = 1; close().catch(console.error); });
  browser.on('exit', code => { if (code) { console.error(`浏览器退出码 ${code}`); process.exitCode = 1; close().catch(console.error); } });
  await once(browser, 'spawn');
  await new Promise(resolve => setTimeout(resolve, 150));
  if (browser.exitCode && browser.exitCode !== 0) throw new Error(`浏览器退出码 ${browser.exitCode}`);
  console.log(app.url);
  schedule();
} catch (error) {
  console.error(`filex Markdown: ${error.message}`);
  process.exitCode = 1;
  await close();
}
