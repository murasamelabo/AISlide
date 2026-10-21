import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { parseArgs } from 'node:util';
import { createHash } from 'node:crypto';
import { unzipSync, zipSync, strFromU8 } from 'fflate';
import { AislideClient } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';
import { publishNewBundle } from './atomic-output.mjs';

const { values } = parseArgs({ options: { out: { type: 'string', default: '.artifacts\\javascript-expression-lab' } } });
const directory = resolve(values.out);
await mkdir(directory, { recursive: true });
const source = '架空データ・説明用 / 外部通信なし。静的PPTXは説明用の代替表示で、実行中の画面とは異なります。';
const definitions = [
  ['simulator', '条件を動かすと、数字が動く', '01 / LIVE CALCULATION',
    ['台数・利用回数・料金を変更', 'JavaScriptで売上・費用・差額をその場で計算する。事前に用意した画像の切り替えではない。', '計算式と値を照合する。これは単純な仮定で、需要予測ではない。'],
    ['生の計算', '台数×利用回数×料金を計算し、表示と棒の長さを更新する。', '範囲外の入力は認めない。計算は端末内だけで完結。'],
    ['数値の説明', '同じ入力から同じ結果が出る。仮定・単位・費用式を画面に残す。', '自由な業務ロジックへ拡張可能だが、このデモは固定式。']],
  ['network', '関係をたどると、影響が見える', '02 / RELATIONSHIP EXPLORER',
    ['ノードを選択', '入口・認証・決済・通知の関係を視覚化。ノードを選び、説明と隣接関係を見る。', 'ポインターとキーボードで選択できる。'],
    ['状態を変える', '選択ノードの停止を仮定し、つながる先の影響表示を更新する。', '実システムの監視・障害操作ではない。'],
    ['SVGの表現', '線・ノード・強調をベクターで描く。拡大しても文字と線を読みやすい。', '外部システムやネットワークには接続しない。']],
  ['priority', 'カードを動かして、考えを整理', '03 / DIRECT MANIPULATION',
    ['ドラッグで配置', '施策カードをボード上で動かし、実行負担と期待する影響を比較する。', '座標は議論用の仮置き。実測の効果を意味しない。'],
    ['キーボードでも', 'カード選択後、矢印キーまたはスライダーで位置を変えられる。', 'ドラッグできない環境でも操作を継続する。'],
    ['編集は一時的', '現在位置を画面で確認。Resetと再読込で初期配置へ戻る。', 'PPTXや外部ファイルへ自動保存しない。']],
  ['playback', '時間をつまんで、流れを説明', '04 / ANIMATION & SCRUBBING',
    ['時点を選択', '時間スライダーを前後に動かし、進行状況とマーカーを連動させる。', '録画動画ではなく、JavaScriptによる動的描画。'],
    ['再生・一時停止', 'requestAnimationFrameで時間を進め、途中停止や再開を試す。', '自動再生せず、非表示時は停止する。'],
    ['段階を伝える', '時間と状態を結び付け、工程の流れを表現する。', '実際の処理時間・速度を示す測定ではない。']],
  ['comparison', '境界を動かして、前後を比べる', '05 / BEFORE–AFTER REVEAL',
    ['境界をドラッグ', '変更前と変更案を重ね、境界位置を動かして比較する。', 'イラストは架空の画面。実製品の画面ではない。'],
    ['見せる範囲を制御', 'clip-path等で表示範囲だけを変える。静止画でも対話の素材になる。', '隠した領域の情報がファイルから削除されるわけではない。'],
    ['操作方法を選ぶ', 'スライダーやキーボードでも境界を移動できる。', '狭い画面でも主要な比較を見失わない。']],
  ['quiz', '選択で説明の道筋を変える', '06 / BRANCHING STORY',
    ['一つ目の判断', '小さく試すか、一度に広げるか。選択に応じて次の説明を変える。', '知識を試す練習用。正解率の測定ではない。'],
    ['次の問いへ', '二段階の選択から、判断理由と注意点を表示する。', '前の段階に戻り、別の分岐を試す。'],
    ['学習に使う', '事例演習、相談の分岐、手順の確認などに応用できる。', '回答は外部送信も集計もされない。']],
  ['explorer', '表を読むより、必要な行を探す', '07 / SEARCH, FILTER & SORT',
    ['検索と絞り込み', '地域名と分類で架空の8レコードを絞り込む。', '該当なしを空白にせず、明示する。'],
    ['並べ替えと詳細', '利用件数で並べ替え、地域を選択して説明を見る。', '表示の変化はブラウザー内で計算する。'],
    ['表の意味を保つ', '期間・単位・満足度の母数を画面で確認できる。', '架空の回答100件を仮定し、実データと誤認させない。']],
  ['canvas', '動く点群で、描画の自由度を知る', '08 / CANVAS DRAWING',
    ['点の数を変更', 'Canvas上に描く点の数を変更し、動きと密度の違いを見る。', '点の位置は合成。センサーや実測値ではない。'],
    ['直接選択', '点の近くをクリックして選択する。選択リストでも同じ点を指定できる。', 'Canvasだけに情報を閉じ込めず、文字で選択結果を示す。'],
    ['再生を制御', '開始・停止を選び、画面サイズに合わせて再描画する。', 'WebGL・外部ライブラリなし。省電力や動きへの配慮も必要。']],
];
const theme = { name: 'JavaScript Expression Lab',
  colors: { dk1: '14263D', lt1: 'FFFFFF', dk2: '52647A', lt2: 'EAF0F8', accent1: '007F78', accent2: 'C85B47', accent3: '4266B1', accent4: 'A86B0C', accent5: '7856A8', accent6: '1D6C8A', hlink: '005FB8', folHlink: '7856A8' },
  fonts: { major: 'Yu Gothic', minor: 'Yu Gothic', east_asian: 'Yu Gothic', complex_script: 'Arial' } };
const client = new AislideClient(requestCore);
const session = await client.createPresentation('javascript-expression-lab', 'JavaScriptで広がる対話スライド / 架空例', { setup: { theme } });
await session.editSlides([{ op: 'rename', slide_id: 'slide-1', title: definitions[0][1] },
  ...definitions.slice(1).map((page, index) => ({ op: 'insert', id: `slide-${index + 2}`, after: `slide-${index + 1}`, title: page[1] }))]);
const pages = [];
for (const [index, [kind, title, subtitle, ...cards]] of definitions.entries()) {
  const slide_id = `slide-${index + 1}`, element_id = `lab-${kind}`;
  const spec = { version: 1, preset: 'interactive/summary', title, subtitle: `${subtitle} / 静的代替：操作と仕組みの説明`,
    layout: { x: 0, y: 0, width: 1280, height: 720, show_title: true },
    data: { kind: 'interactive', spec: { version: 1, template: 'summary', appearance: 'showcase', source,
      groups: [], dimensions: [], items: cards.map(([label, detail, action], i) => ({ id: `concept-${i + 1}`, label, detail, action, metrics: [] })),
      initial_state: { selected_id: 'concept-1', dimension: 0, scale: 'linear' } } } };
  await session.addPart(slide_id, { id: element_id, spec });
  await session.applyOperations([{ op: 'update_notes', slide_id, notes: `JavaScript実験 ${kind}。対話HTMLの操作と、静的PPTXの説明は別表示です。\n${cards.map(card => card.join(' / ')).join('\n')}\nデータは架空。ブラウザー検証とPowerPointホスト内の実操作確認を区別する。` }]);
  pages.push({ kind, title, slide_id, element_id, cards, spec, html: `${String(index + 1).padStart(2, '0')}-${kind}.html` });
}
const before = structuredClone(session.document);
const editable = await session.exportPresentation();
const result = await session.exportCopilotInteractive({ acknowledged_experimental: true, javascript_labs: pages.map(({ slide_id, element_id, kind }) => ({ slide_id, element_id, kind })) });
assert.equal(result.javascript_lab_components, 8);
assert.deepEqual(session.document, before);
const packageParts = unzipSync(Buffer.from(result.base64, 'base64'));
const files = [{ filename: 'javascript-lab-interactive.pptx', bytes: Buffer.from(result.base64, 'base64') },
  { filename: 'javascript-lab-explanatory.pptx', bytes: Buffer.from(editable.base64, 'base64') }];
const decode = value => value.replace(/&(quot|apos|amp|lt|gt|#10|#13|#9);/g, (_, key) => ({ quot: '"', apos: "'", amp: '&', lt: '<', gt: '>', '#10': '\n', '#13': '\r', '#9': '\t' })[key]);
for (const [index, page] of pages.entries()) {
  const xml = strFromU8(packageParts[`ppt/webextensions/webextension${index + 1}.xml`]);
  const property = xml.match(/<we:property name="contentBlockHtml" value="([^"]*)"\/>/);
  if (!property) throw new Error(`Missing own generated lab HTML: ${page.kind}`);
  const html = JSON.parse(decode(property[1]));
  assert.equal(typeof html, 'string');
  assert.ok(html.includes('mountJavascriptLab'));
  files.push({ filename: page.html, bytes: Buffer.from(html) });
}
const rows = pages.map((page, i) => `<li><a href="${page.html}">${String(i + 1).padStart(2, '0')} / ${page.title}</a><p>${page.cards[0][1]}</p></li>`).join('');
files.push({ filename: 'index.html', bytes: Buffer.from(`<!doctype html><html lang="ja"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"><title>JavaScript表現実験室</title><style>body{margin:40px auto;max-width:1100px;padding:20px;background:#14263d;color:white;font:18px/1.7 'Yu Gothic',sans-serif}h1{font-size:36px}ol{padding:0;display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:16px}li{list-style:none;background:#203a56;padding:22px}a{color:#7ce4d8}a:focus-visible{outline:3px solid white;outline-offset:4px}p{margin:8px 0}</style></head><body><h1>JavaScriptで、どこまで表現できる？</h1><p>8つの動く実験。計算、関係探索、ドラッグ、時間再生、比較、分岐、検索、Canvas描画をオフラインで試せます。</p><ol>${rows}</ol><p>全て架空例。通信・外部サービス・端末内のファイル読取り・自動保存はしません。再読込で初期化します。PowerPoint/Copilot内の実再生は未検証です。静的PPTXは操作の説明用で、HTMLと同一画面ではありません。</p></body></html>`) });
files.push({ filename: 'javascript-lab-offline.zip', bytes: Buffer.from(zipSync(Object.fromEntries(files.filter(file => file.filename.endsWith('.html')).map(file => [file.filename, file.bytes])), { level: 6 })) });
const manifest = { format: 'aislide.javascript-expression-lab', version: 1, pages, synthetic: true, data_source: source, network_required: false,
  static_fallback: 'explanatory summary; not the live calculated UI', powerpoint_host_interaction_verified: false, catalog_id: result.catalog_id, warnings: result.warnings,
  files: files.map(file => ({ filename: file.filename, bytes: file.bytes.length, sha256: createHash('sha256').update(file.bytes).digest('hex') })) };
files.push({ filename: 'javascript-lab.manifest.json', bytes: Buffer.from(JSON.stringify(manifest, null, 2)) });
console.log(JSON.stringify({ ...(await publishNewBundle(directory, files)), pages: pages.length, powerpoint_host_interaction_verified: false, warnings: result.warnings }, null, 2));
