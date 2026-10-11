import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm, chmod } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { pathToFileURL } from 'node:url';

const root = new URL('../', import.meta.url).pathname;
const toolkit = new URL('../../toolkit/', import.meta.url).pathname;
const { chromium } = await import(pathToFileURL(join(toolkit, 'node_modules/playwright-core/index.mjs')));
const folder = await mkdtemp(join(tmpdir(), 'filex-markdown-'));
const document = join(folder, "notes 中文 ' ; $(literal).md");
const other = join(folder, 'second.md');
const record = join(folder, 'browser-args.json');
const executable = join(folder, 'firefox');
const children = [];
let browser;
try {
  await writeFile(document, '# Filex preview\n\n**Rendered** 中文\n');
  await writeFile(other, '# Independent preview\n');
  await writeFile(executable, `#!${process.execPath}\nrequire('fs').writeFileSync(${JSON.stringify(record)}, JSON.stringify(process.argv.slice(2)));\n`);
  await chmod(executable, 0o700);
  async function start(path, idle = 30) {
    const child = spawn(process.execPath, [join(root, 'scripts/open_markdown.mjs'), '--toolkit', toolkit,
      '--browser', executable, '--idle-seconds', String(idle), '--', path], { stdio: ['ignore', 'pipe', 'pipe'] });
    children.push(child);
    let errors = '';
    child.stderr.on('data', data => errors += data);
    const url = await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('preview startup timed out: ' + errors)), 10000);
      child.stdout.once('data', data => { clearTimeout(timeout); resolve(data.toString().trim()); });
      child.once('exit', code => { clearTimeout(timeout); reject(new Error('preview exited: ' + code + ' ' + errors)); });
    });
    assert.match(url, /^http:\/\/127\.0\.0\.1:\d+\/$/);
    return { child, url };
  }
  const first = await start(document);
  browser = await chromium.launch({ executablePath: process.env.TOOLKIT_CHROMIUM || '/usr/bin/chromium', headless: true });
  const page = await browser.newPage();
  const failures = [];
  page.on('pageerror', error => failures.push(error.message));
  await page.goto(first.url);
  await page.waitForFunction(path => document.getElementById('filepath').value === path, document);
  const preview = page.frameLocator('#preview');
  await preview.getByRole('heading', { name: 'Filex preview', exact: true }).waitFor();
  assert.equal(await preview.locator('strong').innerText(), 'Rendered');
  const args = JSON.parse(await readFile(record));
  assert.deepEqual(args, ['--new-tab', first.url]);
  await writeFile(document, '# Saved update\n\n**Updated**\n');
  await preview.getByRole('heading', { name: 'Saved update', exact: true }).waitFor();
  const second = await start(other);
  assert.notEqual(second.url, first.url);
  const secondPage = await browser.newPage();
  await secondPage.goto(second.url);
  await secondPage.frameLocator('#preview').getByRole('heading', { name: 'Independent preview', exact: true }).waitFor();
  assert.equal(await page.locator('#filepath').inputValue(), document);
  assert.deepEqual(failures, []);
  const stopping = once(second.child, 'exit');
  second.child.kill('SIGTERM');
  const [stopped] = await stopping;
  assert.equal(stopped, 0);
  await secondPage.close();
  const idle = await start(other, 1);
  const [code] = await once(idle.child, 'exit');
  assert.equal(code, 0);
  assert.equal(await readFile(document, 'utf8'), '# Saved update\n\n**Updated**\n');
  console.log('Toolkit rendered target path, followed saved changes, kept previews independent, passed Firefox URL arguments and idle cleanup.');
} finally {
  await browser?.close();
  for (const child of children) {
    if (child.exitCode === null) {
      child.kill('SIGTERM');
      await once(child, 'exit');
    }
  }
  await rm(folder, { recursive: true, force: true });
}
