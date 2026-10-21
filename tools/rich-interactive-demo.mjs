import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { parseArgs } from 'node:util';
import { createHash } from 'node:crypto';
import { unzipSync, zipSync, strFromU8 } from 'fflate';
import { AislideClient } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';
import { publishNewBundle } from './atomic-output.mjs';

const { values } = parseArgs({ options: { out: { type: 'string', default: '.artifacts\\rich-interactive-showcase' } } });
const directory = resolve(values.out);
await mkdir(directory, { recursive: true });
const source = '架空データ / 各ページは独立した説明用の設定。実績・予測・導入効果ではありません。';
const metric = (label, value, qualifier) => ({ label, value, qualifier });
const item = (id, label, detail, action, metrics = [], extra = {}) => ({ id, label, detail, action, metrics, ...extra });
const pages = [
  {
    id: 'orientation', template: 'summary', title: '触ると、説明の順番が変わる', subtitle: '01 / INTERACTIVE ATLAS — まずは好きなカードを選ぶ',
    task: '任意のカードを選び、右側の説明・大きな数値・次の操作が同時に変わることを確認する。',
    items: [
      item('focus', '要点を掘り下げる', '気になる視点から読む。\n一枚の中で、結論から根拠へ進む。', 'カードを選択 → 右の説明を読む。\nReset で最初の視点へ戻る。', [metric('表示形式', 'カード', '説明の順序は自由')]),
      item('numbers', '数字を探索する', '同じデータを絞り込み、線形・対数の見え方を比べる。', '3枚目で Group と Scale を切り替える。', [metric('切替軸', '2種類', '群とスケール')]),
      item('stages', '段階を進める', '工程を一つずつ開き、その時点の条件を確かめる。', '4枚目の Previous / Next を操作する。', [metric('工程', '6段階', '日付比例ではない')]),
      item('axes', '比較軸を変える', 'コスト・運用・体験など、判断の物差しを入れ替える。', '5枚目で Dimension を切り替える。', [metric('比較軸', '4軸', '定性比較')]),
      item('priorities', '優先度を探る', 'グループごとに施策を整理し、選択した理由を読む。', '6枚目で気になる施策を選ぶ。', [metric('施策', '8件', '4つの判断領域')]),
      item('scenarios', '条件で判断を変える', '三つの仮定を比較する。\n数値は計算結果ではなく、説明用の設定値。', '7枚目でシナリオを切り替える。', [metric('仮定', '3案', '予測・保証ではない')]),
    ],
  },
  {
    id: 'executive', template: 'summary', title: 'AURORA は、どこから育てる？', subtitle: '02 / EXECUTIVE EXPLORER — 架空の新サービスを4つの視点で読む',
    task: '体験・運用・収支・展開を順に選び、KPIと推奨する確認事項の違いを読む。',
    groups: [{ id: 'experience', label: '利用体験' }, { id: 'business', label: '事業設計' }],
    items: [
      item('experience', '体験を磨く', '予約から返却までを短くする。\n利用者が迷う接点を先に検証する。', '次の一手：予約画面の観察テスト。\n完了率だけで使いやすさを断定しない。', [metric('想定利用', '2,000件', '月間の設定値'), metric('検証接点', '4接点', '予約・解錠・返却等')], { group_id: 'experience' }),
      item('operations', '運用を整える', '車両だけでなく、点検と再配置を設計する。\n例外時の担当者を明確にする。', '次の一手：故障・返却遅延の机上演習。\n未検証の復旧時間は約束しない。', [metric('配置拠点', '12か所', '仮の拠点数'), metric('点検頻度', '毎日', '運用案')], { group_id: 'experience' }),
      item('economics', '収支を見極める', '売上だけでなく保守費・再配置費を見る。\n黒字化の時期はまだ決めない。', '次の一手：費用項目の洗い出し。\nこのデモに実際の採算根拠はない。', [metric('検討期間', '90日', '試行計画の案'), metric('費用分類', '3区分', '車両・保守・運用')], { group_id: 'business' }),
      item('rollout', '展開を選ぶ', '広く始めるより、条件を揃えた小規模試行。\n次の地域へ進む基準を先に決める。', '次の一手：継続・中止の基準を合意。\n架空の計画を承認済みと扱わない。', [metric('初期エリア', '2地域', '比較用の仮定'), metric('判断点', '3回', '試行中のレビュー')], { group_id: 'business' }),
    ],
  },
  {
    id: 'dashboard', template: 'dashboard', title: '大きな数字の陰に、何がある？', subtitle: '03 / DATA LENS — 同じ架空データを群とスケールで見直す',
    task: '全群→週末、線形→対数を切り替える。値自体は変わらず、棒の見え方だけが変わることを確認する。',
    groups: [{ id: 'weekday', label: '平日' }, { id: 'weekend', label: '週末' }],
    items: [
      item('station-weekday', '駅前 / 平日', '駅前の設定値が最も大きい。\n小さなカテゴリを読むには対数表示も使う。', '比較条件：各カテゴリは同じ4週間。\n件数の大きさを施策効果と解釈しない。', [metric('利用件数', '12,000', '4週間の設定値'), metric('構成比', '72.2%', '平日 16,620件中')], { group_id: 'weekday', value: 12000 }),
      item('office-weekday', '業務街 / 平日', '通勤・業務移動の想定。\n時間帯の偏りは別に調べる必要がある。', '次の問い：朝夕の車両不足はあるか。\n件数だけでは利用目的は証明できない。', [metric('利用件数', '4,200', '4週間の設定値'), metric('構成比', '25.3%', '平日 16,620件中')], { group_id: 'weekday', value: 4200 }),
      item('park-weekday', '公園 / 平日', '全体では小さく見えるカテゴリ。\n対数では差を読みやすくできる。', '対数表示でも値は420件のまま。\n棒の長さの比を件数比と読まない。', [metric('利用件数', '420', '4週間の設定値'), metric('構成比', '2.5%', '平日 16,620件中')], { group_id: 'weekday', value: 420 }),
      item('station-weekend', '駅前 / 週末', '週末の駅前は平日より小さく置いた。\nこれは観測ではなくシナリオ設定。', 'Group で週末だけを表示する。\n絞り込み後は最大値が変わる。', [metric('利用件数', '7,200', '4週間の設定値'), metric('構成比', '56.0%', '週末 12,850件中')], { group_id: 'weekend', value: 7200 }),
      item('park-weekend', '公園 / 週末', '週末に公園需要が増える仮定。\n平日との差を予測精度と混同しない。', '次の問い：天候の影響をどう分けるか。\n季節・天候はこの設定に含めていない。', [metric('利用件数', '5,600', '4週間の設定値'), metric('構成比', '43.6%', '週末 12,850件中')], { group_id: 'weekend', value: 5600 }),
      item('night-weekend', '夜間 / 週末', '少数の50件もゼロではない。\n対数表示では確認しやすい。', 'ゼロも扱える log(1 + 値) を使用。\n安全性や必要性は別の根拠で判断する。', [metric('利用件数', '50', '4週間の設定値'), metric('構成比', '0.4%', '週末 12,850件中')], { group_id: 'weekend', value: 50 }),
    ].map(item => ({ ...item, unit: '件' })),
  },
  {
    id: 'timeline', template: 'timeline', title: 'ローンチまでを、段階ごとに開く', subtitle: '04 / STAGE EXPLORER — 6つの工程と確認条件',
    task: '工程を選択、Nextで進め、Previousで戻る。工程番号・説明・確認事項が一緒に切り替わることを確認する。',
    items: [
      item('discover', '発見', '誰のどんな移動を支えるのか。\n利用場面を観察し、課題の仮説を置く。', '確認事項：実際の移動と想定を分ける。', [metric('観察対象', '20人', '計画上の設定値')]),
      item('design', '設計', '予約・解錠・返却をつなぐ。\nエラー時の案内も同時に設計する。', '確認事項：通常操作と例外経路を揃える。', [metric('主要接点', '4接点', '説明用の工程案')]),
      item('pilot', '試行', '限定した地域と期間で確かめる。\n合否を後から都合よく変えない。', '確認事項：試行前に判断基準を決める。', [metric('期間', '30日', '試行計画の仮定')]),
      item('review', '評価', '利用件数だけでなく、未完了や問い合わせも読む。\n不足する観測は不足のまま残す。', '確認事項：件数・期間・母数を揃える。', [metric('評価軸', '4軸', '体験・運用・費用等')]),
      item('launch', '公開', '担当者と停止条件を確認して開始する。\n段階を進めるだけで実環境は変わらない。', '確認事項：このデモは本番公開しない。', [metric('対象地域', '2地域', '架空の公開案')]),
      item('learn', '改善', '実際の観測から、次に変えることを決める。\n継続しない判断も選択肢に含める。', '確認事項：継続・修正・中止を区別する。', [metric('見直し', '毎週', '運用案・実績ではない')]),
    ],
  },
  {
    id: 'comparison', template: 'comparison', title: '同じ案でも、物差しで見え方が変わる', subtitle: '05 / COMPARISON STUDIO — 採点せず、判断条件を並べる',
    task: 'Dimensionで4軸を切り替え、候補を選ぶ。定性説明が入れ替わり、選択案の制約が右側に出ることを確認する。',
    dimensions: ['立ち上げ', '運用負担', '利用体験', '拡張性'],
    items: [
      item('small', '小規模から', '小さく学ぶ案。\n対象を絞るので、広域の需要は確かめられない。', 'この案が合う条件：仮説が多く、\nやり直しの余地を残したい。', [metric('対象', '2拠点', '説明用の仮定')], { values: ['対象を限定し、短く準備', '担当範囲を狭く保つ', '利用できる地域は限定', '観測を見て段階的に追加'] }),
      item('partner', '提携して', '地域の運営者と組む案。\n役割・費用・責任の合意が必要。', 'この案が合う条件：地域の運営力を\n活かし、責任分担を決められる。', [metric('運営', '協業', '合意済みではない')], { values: ['相手先との調整が必要', '役割の境界を明確にする', '地域の拠点を活用できる', '提携条件に合わせて拡張'] }),
      item('direct', '自社で広く', '自社で体験を揃える案。\n広域の配置・点検を担う負担も大きい。', 'この案が合う条件：運営体制と費用を\n継続して確保できる。', [metric('運営', '自社', '説明用の選択肢')], { values: ['広い範囲の準備が必要', '点検・再配置を自社で担う', '共通の体験を設計しやすい', '運営資源の増強が前提'] }),
      item('wait', '始めない', '準備と観測を先にする案。\n導入しない選択を比較から外さない。', 'この案が合う条件：重要な前提が未確認。\n不足する根拠を先に集める。', [metric('判断', '保留', '消極策とは限らない')], { values: ['必要な根拠を先に集める', '新規運営はまだ発生しない', '新しい移動体験は提供しない', '開始条件を整えるまで待つ'] }),
    ],
  },
  {
    id: 'priorities', template: 'map', title: '施策を並べ、着手する理由を探る', subtitle: '06 / PRIORITY MAP — 座標のない、4領域の判断ボード',
    task: '8件の施策を選択し、領域・根拠・次の行動を読む。Groupで一領域に絞り込み、Resetで全体に戻る。',
    groups: [{ id: 'now', label: '今すぐ確かめる' }, { id: 'prepare', label: '準備して進める' }, { id: 'explore', label: '小さく探索する' }, { id: 'hold', label: '条件が揃うまで待つ' }],
    items: [
      item('return', '返却を観察', '利用者が返却場所で迷うかを見る。\n問い合わせ件数だけでは原因は分からない。', '次の行動：観察項目を決めて試す。', [metric('方法', '観察', '実施前の案')], { group_id: 'now' }),
      item('maintenance', '点検を確認', '点検の担当と実施記録を揃える。\n稼働率と安全確認を同一視しない。', '次の行動：点検フローの机上確認。', [metric('担当', '運営', '架空の役割')], { group_id: 'now' }),
      item('support', '窓口を設計', '通常の質問と緊急の連絡を分ける。\n対応時刻の約束はまだ置かない。', '次の行動：問い合わせの分類案を作る。', [metric('検討', '連絡経路', '未運用の案')], { group_id: 'prepare' }),
      item('capacity', '配置を計画', '朝夕の偏りを仮説として置く。\n再配置が本当に必要か観測する。', '次の行動：時間帯別の観測を準備。', [metric('検討', '需給', '未観測の仮説')], { group_id: 'prepare' }),
      item('rain', '雨天を調べる', '天候による変化を小さく調べる。\nこのデモの件数に天候効果は含まれない。', '次の行動：天候と利用の記録方法を決める。', [metric('論点', '天候', '探索の対象')], { group_id: 'explore' }),
      item('bundle', '連携を試す', '地域施設との連携を検討する。\n提携済みであるかのように見せない。', '次の行動：提携の前提条件を整理。', [metric('状態', '構想', '契約・合意なし')], { group_id: 'explore' }),
      item('wide', '広域展開', '費用と運営体制を確認してから判断する。\n対象を増やすほど良いとは限らない。', '再検討条件：継続できる運営体制。', [metric('条件', '体制', '達成済みではない')], { group_id: 'hold' }),
      item('dynamic', '料金を変動', '需要に応じて価格を変える案。\n公平性と利用者への説明が課題。', '再検討条件：規約・説明・影響の検証。', [metric('条件', '検証', '未実装の構想')], { group_id: 'hold' }),
    ],
  },
  {
    id: 'scenarios', template: 'summary', title: '条件を変えたら、何を選ぶ？', subtitle: '07 / DECISION LAB — 数字は操作説明の仮定。シミュレーターではありません',
    task: '3つのカードを比較する。条件・設定値・判断が連動して変わるが、数式による予測ではないことを確認する。',
    items: [
      item('steady', '慎重に学ぶ', '少ない拠点で観察を厚くする。\n広域の需要や季節変動は確かめられない。', '判断案：観察を優先。\n進む条件は運営負担と体験の確認。', [metric('拠点', '2', '仮の配置数'), metric('利用', '800件', '月間の設定値'), metric('予算', '120万円', '架空の上限')]),
      item('balanced', '範囲を広げる', '複数の地域条件を比べる。\n地域差と運営差を混同しない設計が必要。', '判断案：比較条件を優先。\n地域ごとの役割と記録を揃える。', [metric('拠点', '6', '仮の配置数'), metric('利用', '2,000件', '月間の設定値'), metric('予算', '300万円', '架空の上限')]),
      item('ambitious', '体制を先に作る', '広い運営体制を先に確保する案。\n利用が増える保証はなく、費用が先行する。', '判断案：未確認の需要を先に検証。\n予算増を成功の根拠にしない。', [metric('拠点', '12', '仮の配置数'), metric('利用', '3,600件', '月間の設定値'), metric('予算', '620万円', '架空の上限')]),
    ],
  },
];

const theme = {
  name: 'Aurora Editorial',
  colors: { dk1: '14263D', lt1: 'FFFFFF', dk2: '52647A', lt2: 'EAF0F8', accent1: '007F78', accent2: 'C85B47', accent3: '4266B1', accent4: 'A86B0C', accent5: '7856A8', accent6: '1D6C8A', hlink: '005FB8', folHlink: '7856A8' },
  fonts: { major: 'Yu Gothic', minor: 'Yu Gothic', east_asian: 'Yu Gothic', complex_script: 'Arial' },
};
const client = new AislideClient(requestCore);
const session = await client.createPresentation('rich-interactive-showcase', 'AURORA / 触って読み解くスライド図鑑', { setup: { theme } });
await session.editSlides([{ op: 'rename', slide_id: 'slide-1', title: pages[0].title },
  ...pages.slice(1).map((page, index) => ({ op: 'insert', id: `slide-${index + 2}`, after: `slide-${index + 1}`, title: page.title }))]);
const records = [];
for (const [index, page] of pages.entries()) {
  const spec = { version: 1, preset: `interactive/${page.template}`, title: page.title, subtitle: page.subtitle,
    layout: { x: 0, y: 0, width: 1280, height: 720, show_title: true },
    data: { kind: 'interactive', spec: { version: 1, template: page.template, appearance: 'showcase', source,
      groups: page.groups ?? [], dimensions: page.dimensions ?? [], items: page.items,
      initial_state: { selected_id: page.items[0].id, dimension: 0, scale: 'linear' } } } };
  const payload = await client.compileInteractive({ id: `rich-${page.id}`, spec, theme });
  console.log(`Compiled ${page.id}: ${payload.views.length} interaction states`);
  await session.addPart(`slide-${index + 1}`, { id: `rich-${page.id}`, spec });
  await session.applyOperations([{ op: 'update_notes', slide_id: `slide-${index + 1}`,
    notes: `${page.task}\n全て架空例。実測値・予測値・導入効果を示さない。Group は分類、Scale は表示スケール、Dimension は比較軸。操作は閲覧状態のみを変更し保存しない。\nPowerPoint内の実クリック再生は別途確認が必要。` }]);
  records.push({ id: page.id, slide_id: `slide-${index + 1}`, role: index === 0 ? 'orientation' : 'interactive-demonstration', title: page.title, task: page.task, evidence: 'synthetic-scenario', spec, compiled_states: payload.views.length });
}
const before = structuredClone(session.document);
const editable = await session.exportPresentation();
const playback = await session.exportCopilotInteractive({ acknowledged_experimental: true });
assert.deepEqual(session.document, before);
const packageParts = unzipSync(Buffer.from(playback.base64, 'base64'));
const items = [
  { filename: 'aurora-interactive.pptx', bytes: Buffer.from(playback.base64, 'base64') },
  { filename: 'aurora-editable.pptx', bytes: Buffer.from(editable.base64, 'base64') },
];
function xmlAttribute(value) {
  return value.replace(/&(quot|apos|amp|lt|gt|#10|#13|#9);/g, (_, code) => ({ quot: '"', apos: "'", amp: '&', lt: '<', gt: '>', '#10': '\n', '#13': '\r', '#9': '\t' })[code]);
}
for (const [index, page] of pages.entries()) {
  const xml = strFromU8(packageParts[`ppt/webextensions/webextension${index + 1}.xml`]);
  const property = xml.match(/<we:property name="contentBlockHtml" value="([^"]*)"\/>/);
  if (!property) throw new Error(`Generated Copilot property missing: ${page.id}`);
  const html = JSON.parse(xmlAttribute(property[1]));
  assert.equal(typeof html, 'string');
  items.push({ filename: `${String(index + 1).padStart(2, '0')}-${page.id}.html`, bytes: Buffer.from(html) });
}
const ledger = { primary_purpose: 'product-demo', secondary_purpose: null, language: 'ja', audience: '対話型の表現を評価する資料制作者', goal: '操作で変わる視点・説明・判断を比較する', delivery: 'hands-on / PowerPoint and offline HTML', synthetic: true, powerpoint_playback_verified: false, source, theme, slides: records };
items.push({ filename: 'aurora-authoring.json', bytes: Buffer.from(JSON.stringify(ledger, null, 2)) });
const links = pages.map((page, index) => `<li><a href="${String(index + 1).padStart(2, '0')}-${page.id}.html">${String(index + 1).padStart(2, '0')} / ${page.title}</a><p>${page.task}</p></li>`).join('');
items.push({ filename: 'index.html', bytes: Buffer.from(`<!doctype html><html lang="ja"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'"><title>AURORA / 対話型スライド図鑑</title><style>body{max-width:1000px;margin:48px auto;padding:24px;background:#14263d;color:#fff;font:18px/1.8 'Yu Gothic',sans-serif}h1{font-size:36px}a{color:#79e4d6}li{background:#213b57;margin:16px 0;padding:20px;list-style:none}ol{padding:0}p{margin:8px 0}a:focus-visible{outline:3px solid #fff;outline-offset:4px}</style></head><body><h1>触って読み解くスライド図鑑</h1><p>AURORA / 架空データによる7つのデモ。ページを選ぶとオフラインで操作できます。ブラウザー操作はPowerPoint内の実再生の保証ではありません。</p><ol>${links}</ol><p>比較・数値は説明用の仮定です。実績、予測、効果の保証を示しません。操作は保存されず、開き直すと既定状態に戻ります。</p></body></html>`) });
items.push({ filename: 'aurora-offline-html.zip', bytes: Buffer.from(zipSync(Object.fromEntries(items.filter(file => file.filename.endsWith('.html')).map(file => [file.filename, file.bytes])), { level: 6 })) });
const receipt = { format: 'aislide.rich-interactive-showcase', version: 1, pages: pages.length, total_compiled_states: records.reduce((sum, record) => sum + record.compiled_states, 0), catalog_id: playback.catalog_id, synthetic: true, powerpoint_playback_verified: false, warnings: playback.warnings, limitations: playback.limitations,
  files: items.map(file => ({ filename: file.filename, bytes: file.bytes.length, sha256: createHash('sha256').update(file.bytes).digest('hex') })) };
items.push({ filename: 'aurora.manifest.json', bytes: Buffer.from(JSON.stringify(receipt, null, 2)) });
const publication = await publishNewBundle(directory, items);
console.log(JSON.stringify({ ...publication, pages: pages.length, states: receipt.total_compiled_states, warnings: playback.warnings }, null, 2));
