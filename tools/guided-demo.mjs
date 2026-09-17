import assert from 'node:assert/strict';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const part = (preset, title, data) => ({ version: 1, preset, title, subtitle: '検証用の架空データ・設計案 / 実績や効果保証ではありません', data });
const items = (entries, center = '') => ({ kind: 'items', center, items: entries.map(([label, detail]) => ({ label, detail })) });
function declarations(value, path = '/data') {
  if (typeof value === 'number') return [{ path, value, evidence_id: 'measure' }];
  if (Array.isArray(value)) return value.flatMap((item, index) => declarations(item, `${path}/${index}`));
  if (value && typeof value === 'object') return Object.entries(value).flatMap(([key, item]) => declarations(item, `${path}/${key}`));
  return [];
}
function slide(id, headline, body, form = 'explanation', evidence = ['proposal']) {
  return { id, section: '判断と検証', headline, sentence_form: form, pattern_id: 'native-part', question: 'このページで何を確認するか', parent_message: 'governing', transition: form === 'conditional' ? '条件が整えば' : 'そのため', parallel_basis: '検証対象', part: body, support: [{ clause: headline, body_paths: ['/data'], evidence_ids: evidence }], numbers: declarations(body.data) };
}
function base(profile, title, governing, slides) {
  return { version: 1, profile_id: profile, title, audience: '検証会議の参加者', purpose: '架空の例で構成と編集方法を確認する', governing_message: governing, language: 'ja', evidence: [{ id: 'proposal', kind: 'assumption', reference: '説明用の設計案', statement: '担当と条件を固定して検証する案。' }, { id: 'measure', kind: 'assumption', reference: 'スクリプト内の架空値', statement: '数値は動作検証用の仮定で、実績ではない。' }], slides };
}

export function guidedExamples() {
  const issues = [
    { id: 'Q1', question: '試行対象', requested_decision: '確認工程から試行', criterion: '対象と条件を合意', owner: '業務責任者', due: '次回レビュー', evidence_ids: ['waiting'], analysis_slide_ids: ['waiting-analysis'] },
    { id: 'Q2', question: '責任分担', requested_decision: '窓口と承認者を固定', criterion: '引継ぎ担当を明確化', owner: '運用担当', due: '試行前', evidence_ids: ['ownership'], analysis_slide_ids: ['ownership-analysis'] },
    { id: 'Q3', question: '評価方法', requested_decision: '同条件で結果を比較', criterion: '測定条件を再現', owner: '品質担当', due: '試行中', evidence_ids: ['measurement'], analysis_slide_ids: ['measurement-analysis'] },
  ];
  const decisionPage = (id, pattern, headline, form) => ({ id, section: '決定事項', headline, sentence_form: form, pattern_id: pattern, question: '何を合意すれば検証を開始できるか', parent_message: 'governing', transition: 'したがって', parallel_basis: '決定論点', support: [{ clause: headline, body_paths: ['/issues'], evidence_ids: ['waiting', 'ownership', 'measurement'] }], numbers: [] });
  const consulting = base('consulting-decision', '確認工程の改善を判断する', '確認工程に限定した試行で、改善の条件を検証する。', [
    decisionPage('summary', 'C02', '試行の対象と責任者を先に決めることで、確認工程の改善を検証できる', 'proposal'),
    slide('waiting-analysis', '仮定の処理時間は待ちが大半のため、試行では確認工程を優先すべきだ', part('horizontal-bar-graph/labeled', '処理時間の仮定（分／件）', { kind: 'chart', categories: ['実働', '確認待ち'], series: [{ name: '分／件', values: [8, 22] }], x_axis: '分／件', y_axis: '内訳' }), 'causal', ['waiting']),
    slide('ownership-analysis', '担当が曖昧な引継ぎを残さないため、窓口と承認者を先に固定すべきだ', part('flow/balanced', '試行中の担当と引継ぎ', items([['受付', '窓口が条件を確認'], ['承認', '判断する担当を固定'], ['記録', '結果を同じ形式で残す']])), 'proposal', ['ownership']),
    slide('measurement-analysis', '試行の結果を同じ条件で比較できれば、対象を広げる判断の根拠を揃えられる', part('matrix/labeled', '評価方法と確認条件', { kind: 'matrix', rows: ['処理時間', '差戻し', '展開判断'], columns: ['測定条件', '評価の方法'], cells: [['同じ受付区分', '試行前後を比較'], ['同じ対象期間', '理由と件数を記録'], ['責任者が確認', '条件を満たせば検討']] }), 'conditional', ['measurement']),
    decisionPage('closing', 'C03', '対象と責任者と判定基準を合意すれば、限定試行に着手できる状態を整えられる', 'conditional'),
  ]);
  consulting.issues = issues;
  consulting.evidence.push({ id: 'waiting', kind: 'assumption', reference: '架空の時間内訳', statement: '実働8分、確認待ち22分と仮定。' }, { id: 'ownership', kind: 'assumption', reference: '架空の運用案', statement: '窓口固定で引継ぎを明確にする案。' }, { id: 'measurement', kind: 'assumption', reference: '架空の評価案', statement: '同条件の比較で展開の可否を判断する案。' });
  consulting.slides[1].numbers.forEach((entry) => { entry.evidence_id = 'waiting'; });

  const technical = base('technical-explainer', '処理境界と異常時の動きを説明する', '処理境界ごとに入力と権限を確認する。', [
    slide('boundary', '入力と権限の確認を分ければ、異常を処理の入口で止められる', part('flow/balanced', '提案する検証順序', items([['受付', '入力の形式を確認'], ['認可', '実行の権限を確認'], ['処理', '許可した操作を実行']]))),
    slide('ownership', '機能の責任を親子関係で分けると、変更の影響範囲を確認できる', part('tree/focus', '論理的な責任分担（設計案）', { kind: 'tree', nodes: [{ id: 'root', label: '受付サービス' }, { id: 'contract', label: '入力契約', parent: 'root' }, { id: 'operations', label: '処理管理', parent: 'root' }, { id: 'format', label: '形式検証', parent: 'contract' }, { id: 'permission', label: '権限検証', parent: 'contract' }] })),
    slide('failure', '失敗時の応答を定義しておけば、再試行と人の判断を分離できる', part('matrix/labeled', '失敗の種類と対応方針', { kind: 'matrix', rows: ['入力不備', '権限不足', '一時障害'], columns: ['応答', '次の対応'], cells: [['理由を返す', '入力を修正'], ['実行しない', '権限を確認'], ['状態を記録', '条件付き再試行']] }), 'conditional'),
  ]);
  const event = base('event-talk', '観測から次の行動へつなげる', '観測と解釈を分け、小さく試して学ぶ。', [
    slide('opening', '観測と解釈を分けることで、次に試すべきことが見えてくる', part('list-horizontal/focus', '議論を始めるための視点', items([['観測', '何が起きたか'], ['解釈', 'なぜ重要か'], ['行動', '何を確かめるか']]))),
    slide('learning', '小さく試して結果を共有すれば、次の判断を更新できる', part('cycle/balanced', '学びを次の試行へ戻す', items([['観測', '変化を捉える'], ['検討', '条件を比べる'], ['試行', '小さく試す'], ['共有', '結果を残す']], '継続して学ぶ')), 'conditional'),
    slide('action', '同じ問いを持ち帰れば、日々の観測を具体的な行動につなげられる', part('list-horizontal/labeled', '持ち帰る問い', items([['事実は何か', '観測を確かめる'], ['条件は何か', '前提を言葉にする'], ['何を試すか', '次の一歩を決める']])), 'proposal'),
  ]);
  const report = base('status-report', '架空の実績変動と対応を報告する', '件数の変動を要因別に確認し、対応を決める。', [
    slide('trend', '件数が増えた後に低下しているため、変動要因を確認する必要がある', part('line-graph/labeled', '期間別の件数（架空値）', { kind: 'chart', categories: ['第1期', '第2期', '第3期'], series: [{ name: '件数', values: [100, 120, 115] }], x_axis: '期間', y_axis: '件' }), 'causal', ['measure']),
    slide('bridge', '増加と減少の寄与を分ければ、次回の確認対象を絞り込める', part('water-fall/labeled', '開始値から最終値への内訳（件）', { kind: 'waterfall', unit: '件（架空値）', steps: [{ label: '開始', value: 100, total: true }, { label: '増加要因', value: 20 }, { label: '減少要因', value: -5 }, { label: '最終', value: 115, total: true }] }), 'conditional', ['measure']),
    slide('follow-up', '条件と担当を揃えて確認すれば、次回報告で対応の結果を比較できる', part('list-horizontal/labeled', '次回報告までの確認事項', items([['対象を揃える', '同じ範囲で記録'], ['要因を確認', '担当が理由を確認'], ['結果を残す', '変更と効果を分ける']])), 'proposal'),
  ]);
  return [consulting, technical, event, report];
}

async function main() {
  const directory = resolve(process.argv[2] ?? `.artifacts/guided-${Date.now()}`);
  await mkdir(directory, { recursive: true });
  assert.equal((await readdir(directory)).length, 0, 'Use a new or empty output directory');
  const client = new Client({ name: 'guided-authoring-proof', version: '1.0.0' });
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
  const call = async (name, args = {}) => { const result = await client.callTool({ name, arguments: args }); assert.ok(!result.isError, `${name}: ${JSON.stringify(result.content)}`); return JSON.parse(result.content[0].text); };
  const records = [];
  try {
    await client.connect(transport);
    assert.equal((await call('best_practice_profiles')).profiles.length, 4);
    for (const input of guidedExamples()) {
      const guide = await call('best_practice_guide', { profile_id: input.profile_id });
      assert.equal(guide.language, 'en');
      const checked = await call('validate_guided_presentation', { input });
      assert.equal(checked.ready, true, `${input.profile_id}: ${JSON.stringify(checked.issues)}`);
      const created = await call('create_guided_presentation', { input });
      assert.equal(created.model_inference, false);
      const document = await call('get_document', { deck_id: created.deck_id });
      const filename = `${input.profile_id}.pptx`;
      await call('export_pptx', { deck_id: created.deck_id, filename });
      const bytes = await readFile(join(directory, filename));
      assert.equal(bytes.subarray(0, 2).toString(), 'PK');
      const reopened = await call('open_pptx', { base64: bytes.toString('base64') });
      const restored = await call('get_document', { deck_id: reopened.deck_id });
      assert.equal(restored.deck.slides.length, input.slides.length);
      assert.ok(restored.parts.every((part) => !part.stale));
      const first = restored.parts[0];
      await call('update_part', { deck_id: reopened.deck_id, expected_revision: restored.revision, slide_id: first.slide_id, id: first.element_id, spec: { ...first.spec, title: '編集確認用の見出し' } });
      await call('undo', { deck_id: reopened.deck_id });
      await call('export_pptx', { deck_id: reopened.deck_id, filename: `${input.profile_id}-undo.pptx` });
      assert.deepEqual(await readFile(join(directory, `${input.profile_id}-undo.pptx`)), bytes);
      const objects = {};
      function count(elements) { for (const element of elements) { objects[element.type] = (objects[element.type] ?? 0) + 1; if (element.type === 'group') count(element.children); } }
      for (const slide of document.deck.slides) count(slide.elements);
      await writeFile(join(directory, `${input.profile_id}.json`), JSON.stringify(input, null, 2), { flag: 'wx' });
      records.push({ profile_id: input.profile_id, filename, slides: input.slides.length, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex'), objects, parts: restored.parts.length, metadata_current: true, edit_undo_identical: true, validation: checked });
    }
  } finally { await client.close(); }
  await writeFile(join(directory, 'evidence.json'), JSON.stringify({ generated_at: new Date().toISOString(), mode: 'official MCP SDK / stdio', data: 'Synthetic examples with explicit assumptions', files: records }, null, 2), { flag: 'wx' });
  console.log(JSON.stringify({ directory, files: records }, null, 2));
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) await main();