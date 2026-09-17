import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { basename, join, resolve } from 'node:path';
import sharp from 'sharp';
import { chromium, expect } from '@playwright/test';
import { AislideClient } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';
import { publishNewFile } from './atomic-output.mjs';

assert.equal(process.argv.length, 4, 'Usage: node tools/authoring-verify.mjs <capture-manifest.json> <new-output-directory>');
const manifest = JSON.parse(await readFile(resolve(process.argv[2]), 'utf8'));
assert.ok(Array.isArray(manifest.files) && manifest.files.length > 0 && manifest.files.length <= 16);
const directory = resolve(process.argv[3]);
await mkdir(directory, { recursive: true });
assert.equal((await readdir(directory)).length, 0, 'Use a new or empty output directory');
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');
const client = new AislideClient(requestCore);
const browser = await chromium.launch({ channel: 'msedge', headless: true });
const records = [];
const pageErrors = [];
try {
  const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
  page.on('pageerror', (error) => pageErrors.push(error.message));
  assert.equal((await page.goto('http://127.0.0.1:4174/')).status(), 200);
  await expect(page.getByRole('button', { name: 'Open PPTX', exact: true })).toBeEnabled();
  for (const entry of manifest.files) {
    assert.equal(entry.filename, basename(entry.filename));
    assert.match(entry.filename, /^[a-z0-9_-]+\.pptx$/);
    const evidence = JSON.parse(await readFile(join(resolve(entry.source_directory), 'evidence.json'), 'utf8'));
    const source = evidence.files.find((file) => file.filename === entry.filename);
    assert.ok(source && source.slides >= 1 && source.slides <= 32);
    const bytes = await readFile(join(resolve(entry.source_directory), entry.filename));
    assert.equal(bytes.subarray(0, 2).toString(), 'PK');
    assert.equal(digest(bytes), source.sha256);
    assert.equal(digest(await readFile(join(resolve(entry.capture_directory), 'verification-copy.pptx'))), source.sha256, 'Office captures must belong to these exact PPTX bytes');
    const session = (await client.openPresentation(`verify-${entry.filename}`, bytes.toString('base64'))).session;
    const document = session.document;
    assert.equal(document.deck.slides.length, source.slides);
    assert.ok(document.parts.every((part) => !part.stale));
    assert.equal((await session.exportPresentation()).base64, bytes.toString('base64'));
    const target = join(directory, entry.filename.replace(/\.pptx$/, ''));
    await mkdir(target);
    await mkdir(join(target, 'previews'));
    const composite = [];
    for (let index = 0; index < source.slides; index++) {
      const name = `slide-${String(index + 1).padStart(2, '0')}.png`;
      const image = await readFile(join(resolve(entry.capture_directory), name));
      const metadata = await sharp(image).metadata();
      assert.equal(metadata.width, 1280); assert.equal(metadata.height, 720);
      assert.ok((await sharp(image).removeAlpha().stats()).channels.some((channel) => channel.stdev > 8), `${entry.filename}/${name} must not be blank`);
      await writeFile(join(target, 'previews', name), image, { flag: 'wx' });
      composite.push({ input: await sharp(image).resize(480, 270).png().toBuffer(), left: 16 + index % 3 * 496, top: 16 + Math.floor(index / 3) * 286 });
    }
    await sharp({ create: { width: 1504, height: 16 + Math.ceil(source.slides / 3) * 286, channels: 3, background: '#e8edef' } }).composite(composite).jpeg({ quality: 94 }).toFile(join(target, 'overview.jpg'));
    if (source.profile_id) {
      await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: entry.filename, mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: bytes });
      await expect(page.locator('.thumbnail')).toHaveCount(source.slides);
      await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
      for (const [index, slide] of document.deck.slides.entries()) {
        await page.getByRole('button', { name: `Slide ${index + 1}: ${slide.title}`, exact: true }).click();
        await expect(page.locator('.slide-stage').getByText(slide.title, { exact: true })).toBeVisible();
        await expect(page.locator('.slide-stage [role="status"]')).toHaveCount(0);
        await page.evaluate(() => document.fonts.ready);
        const overflow = await page.locator('.slide-stage .slide-text').evaluateAll((nodes) => nodes.filter((node) => node.scrollWidth > node.clientWidth + 2 || node.scrollHeight > node.clientHeight + 2).map((node) => node.textContent));
        assert.deepEqual(overflow, [], `${source.profile_id} slide ${index + 1}`);
        await expect(page.getByRole('alert')).toHaveCount(0);
      }
      const part = document.parts[0];
      const index = document.deck.slides.findIndex((slide) => slide.id === part.slide_id);
      await page.getByRole('button', { name: `Slide ${index + 1}: ${document.deck.slides[index].title}`, exact: true }).click();
      await page.getByRole('button', { name: `Select ${part.element_id}`, exact: true }).click();
      await page.getByRole('button', { name: 'Edit part data', exact: true }).click();
      const dialog = page.getByRole('dialog', { name: 'Parts library', exact: true });
      await dialog.getByLabel('Part title', { exact: true }).fill('編集確認');
      await dialog.getByRole('button', { name: 'Update part', exact: true }).click();
      await expect(page.locator('.slide-stage').getByText('編集確認', { exact: true })).toBeVisible();
      await page.getByRole('button', { name: 'Undo', exact: true }).click();
      await expect(page.locator('.slide-stage').getByText(part.spec.title, { exact: true })).toBeVisible();
      const pending = page.waitForEvent('download');
      await page.getByRole('button', { name: 'Save PPTX', exact: true }).click();
      assert.deepEqual(await readFile(await (await pending).path()), bytes);
      await page.screenshot({ path: join(target, 'studio.png') });
      await writeFile(join(target, 'input.json'), await readFile(join(resolve(entry.source_directory), `${source.profile_id}.json`)), { flag: 'wx' });
    }
    await publishNewFile(join(target, `.presentation-${randomUUID()}.tmp`), join(target, entry.filename), bytes);
    assert.equal(digest(await readFile(join(resolve(entry.source_directory), entry.filename))), source.sha256);
    records.push({ filename: entry.filename, directory: basename(target), slides: source.slides, sha256: source.sha256, objects: source.objects, presets: source.presets, profile_id: source.profile_id, office_captures: source.slides, office_capture_source_matches: true, metadata_current: true, no_op_identical: true, studio_edit_undo_save_identical: Boolean(source.profile_id) });
  }
  assert.deepEqual(pageErrors, []);
} finally { await browser.close(); }
const report = { verified_at: new Date().toISOString(), scope: 'Generated examples only, not all possible inputs or universal Office parity', data: 'Synthetic values and explicitly declared proposals; not factual business data', files: records };
await writeFile(join(directory, 'verification.json'), JSON.stringify(report, null, 2), { flag: 'wx' });
const guide = `# パーツ改修と用途別MCPの確認資料

全108パーツを日本語で実挿入した4冊と、MCP経由で作成した4用途・14枚のサンプルです。すべて説明用の架空値・設計案で、実績や効果保証ではありません。

| 資料 | 枚数 | PowerPoint描画一覧 | 入力例 |
| --- | --- | --- | --- |
${records.map((record) => `| [${record.filename}](${record.directory}/${record.filename}) | ${record.slides} | [一覧](${record.directory}/overview.jpg) | ${record.profile_id ? `[JSON](${record.directory}/input.json)` : '全27パターン'} |`).join('\n')}

## 使い方

PPTXだけをAISlideのOpen PPTXで開けます。パーツを選びEdit part dataで内容を変更できます。コンサル資料の冒頭サマリと最終決定表は、内容に応じた列幅を持つ編集可能な図形・文字グループです。通常のPowerPoint表とは異なります。

MCPではbest_practice_profilesで用途を一覧し、best_practice_guideで英語ガイド・入力スキーマを取得します。主張と根拠を整理した入力をvalidate_guided_presentationへ渡し、問題を直してからcreate_guided_presentationで新規資料を作成します。export_pptxによる保存は別操作です。

48のコンサル型はすべて選択指針として保存されていますが、48種類すべてが自動作図テンプレートではありません。専用の自動型はC02・C03で、他は既存パーツの組合せまたは手動設計が必要です。readyは入力とレイアウトがコンパイル可能なことを意味し、事実の正しさや主張の妥当性を証明しません。

## 確認範囲

全108既定例の生成・文字枠・PPTX再読込、同じ108パーツを日本語で実挿入したStudio表示、PowerPointでの全ページ描画を確認しました。グラフの埋め込みデータは検証用コピーで編集しました。用途別サンプルはMCPとStudioで編集・Undo・元のPPTXとの完全一致を確認しています。

一覧画像はPowerPointの実描画です。検証コピーのハッシュと元PPTXを照合して対応を確認しています。任意の文字量や特殊な入力すべて、Office表示の完全互換、意味の正しさを保証するものではありません。資料の出典・試算条件・論理台帳はノートに含まれるため、再配布前に確認してください。

再生成はtools/parts-demo.mjs（--japanese）およびtools/guided-demo.mjsを使用します。検証記録は[verification.json](verification.json)にあります。旧ユーザー資料や通常インストール済みアプリは変更していません。
`;
await writeFile(join(directory, 'README.md'), '\uFEFF' + guide, { flag: 'wx' });
console.log(JSON.stringify({ directory, presentations: records.length, slides: records.reduce((sum, entry) => sum + entry.slides, 0), sourceHashesVerified: true, guidedStudioEditUndoSave: true }, null, 2));