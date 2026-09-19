# 限定対応の追加開発

開始日: 2026-09-18。更新日: 2026-09-19。既存の[28項目の実装記録](editing-expansion.md)を引き継ぐ。限定範囲の実装・最終製品検証・通常Windowsデスクトップ更新・通常commit/pushを完了した。既存Publicリポジトリ `murasamelabo/AISlide` の `main` に実装コミット [54548846b1f651adc2ff070f73f122d63aaa77e2](https://github.com/murasamelabo/AISlide/commit/54548846b1f651adc2ff070f73f122d63aaa77e2) を公開し、GitHub上のSHA一致を確認した。以下の限定検証を任意PPTXの完全互換や全制約の解消とは扱わない。

## 依頼と境界

利用者の依頼は、前回答の限定対応機能の追加開発、commit/push、および通常のWindowsデスクトップ版の更新。既存の未コミット変更を維持し、追加分と合わせて検証する。元PPTXの上書き、取り込んだコードの実行、外部関連の自動取得、保護解除、既存PPTXエンジンへの置換は行わない。発表・動画・暗号化・クラウド共同編集の延期を維持する。

計画確認には自律的に進める指示が返された。追加モデルは公開利用条件とハッシュを確認できるものを合計1GB以内でローカル導入し、文書・画像を外部AIへ送信しない。既定largeは256枚・文書32MiB・JSON96MiB・原本16MiBで、standard/legacyの数値上限は変更しない。作業コピーは既定OFF、明示許可時のみ最大5件・48MiB/件・合計100MiB。有効ストアを開く際に保存から7日経過したコピーを期限処理し、旧v1は無断移行・削除しない。

任意の文書の完全互換、全個人情報の検出、WCAG認証、無制限の資源利用は完了条件にしない。対応する形式・演算・検出項目・検証資料を具体的に示す。モデルの推測は人が確認する提案として表示する。

## 実装と受入条件

| 段階 | 対象 | 観測可能な完了条件 | 状態 |
| --- | --- | --- | --- |
| 1 | G35 PDF・印刷 | 日英Unicode検索・選択、読み順・代替説明・明示表ヘッダーのタグ反映、利用者操作によるOS印刷ダイアログ | 限定実装完了・native Print/Cancel確認済み（取消のみ）。可視文字は輪郭、直接印刷は96dpi PNG。[契約](../api.md#static-export-and-recovery) |
| 2 | G10/G11/G18 図形・書式・ベクター | 曲線演算とfragment、7種書式コピー、PNGスポイト、SVG文字・埋込画像、限定EMF/WMFを保存・再読込・Undo | 限定実装完了。曲線は0.25px目標の近似、外部参照は禁止。[範囲](../testing/authoring.md#phase2-visual-tools-2026-09-18) |
| 3 | G14/G15/G24/G25/G27/G28/G29 文書互換 | 追加グラフ描画、8種WordArt、rich notes・付帯master・field継承・CJKの限定往復と未知XML保護 | 限定実装完了。新規・連続再編集の3版をOfficeで確認。8種WordArt・notes・付帯masterの読取りと日本語表示を確認したが、埋込フォント消費・field再計算・任意互換は未証明。[描画](../authoring/chart-wordart-presentation.md)・[notes/fields](../authoring/master-fields-themes.md) |
| 4 | G04/G12 ローカルAI | 実Qwen校正／翻訳と実U2NetPマスク、レビュー・取消・原文SHA/リビジョン検査・単一Undo | 限定実装完了・実モデル確認済み。日本語品質と細部マスクは人の確認が必要、重み非同梱。[実測と制約](../authoring/local-ai.md) |
| 5 | G37/G38 回復・容量 | 明示許可したv2作業コピー、再起動後の両履歴、large容量と全プロファイル296ID制約 | 限定実装完了。最終security修正後のnative 6件と通常インストール済みバイナリの分離fixture 4件が成功し、別プロセス・新規WebViewへの再起動後のUndo/Redoを確認。[上限と証拠](../testing/phase5-recovery-capacity.md) |
| 6 | G40/G41/G43 コメント・検査 | modern thread往復、値を隠したPII候補、明示table_headersのPDF消費と実効theme/alt検査 | 限定実装完了。本文反復保存・順序変更拒否・PDF表親子関係を修正済み。Office読取りで3コメント・3返信を確認。実コメントUI編集・認証通知・text-range投影は未確認／未対応。[契約](../authoring/review-phase6.md) |

各段階は共有コア、Studio、SDK/MCPの必要な入口、失敗時の原子性、保存・再読込・Undoを含む。実装できない受入条件が判明した場合は、削除せず理由と残作業を記録する。

## 対象検証と修正

- G35: 実StructTree/MCID、日英Unicode、読み順・altを保持。表の親子関係修正とunknown/none→TD、first_row→TH/Column、first_column→TH/Row、both角→Bothの11件が成功。結合spanは保持するが複雑な関連付けは保証しない。
- 直接印刷: 同一DOMのReact portalへ最大32枚のPNG blobのみを配置し、印刷時だけアプリを隠す。iframeなし、CSP frame-src noneは変更なし。所有PID/command markerとローカライズ済みPrint/Cancel、beforeprint/afterprintを実機確認しCancelのみ実施、ジョブ送信なし。証拠は `.artifacts/g35-directprint-20260919/native-print-proof.json`、対象web/nativeログはfinal-gatesに記録。
- G04/G12: Qwenは入力・出力各8,000文字の厳密JSONで外部API不使用。追加4.36MiB U2NetP/tract-onnx CPU実推論は前景alpha255・背景0、debug約22～30秒。768MiBは受付時の評価予算でRSS上限ではなく、CPU実行途中の強制中断はしない。モデル・DUTSの権利参照を任意setupで提示し、重みは公開しない。
- G28: 許可済みOFLの静的Noto Sans JP 5,766,884 bytes（約5.77MB）、全17,936glyphを保持したfixtureと約12.15MBの再読込文書を確認。8face・12MiB/face・合計24MiBに加え全文書予算を適用。正確なフォントbytes・native font part・Office日本語描画を確認したが、Officeが埋込フォントを消費した証明ではない。
- コメント／ノート: modern本文の反復再読込でendParaRPrの名前空間を保持し、native配列順序変更は黙って無視せず拒否。ノートのlanguage-only・indent・point spacingを認識し空のplain投影を保持。対象comments/single-file計69件成功。
- 回復／要求制御: 実Windowsで再現したディレクトリTOCTOUをroot/ancestorの固定ハンドルと作成前open-file検査で修正し、storage単体7件成功。非Windows nativeストアはfail-closed。foreground/preview分離、独立Recovery処理、両要求レーンの早期取消保持は64件・TTL30秒。最終native単体14件成功。両履歴は各30receipt/4MiB、保存は世代CASと取消を維持。
- 最新security修正後のnative 6件と通常インストール済みバイナリの分離fixture 4件が成功。旧5 E2E entryは最終値に合算しない。限定静的security再レビューではHigh/Medium残件なし。ただしファイル検査を実施できなかったレビューは証拠に含めず、全面監査とはしない。

## 最終Office確認

ローカル証拠: `.artifacts/final-office-20260919-7e9c41/handoff.json`。SDK 31要求、新規出力から本文の連続native再編集2回まで、3版すべてでexport/reopen成功。Open XML SDK 3.5.1のOffice2021検査は3 PPTX＋3埋込XLSXでエラー0。実Office 16.0 build 20430は合計9スライド・3チャートを検査し、所有コピーのworkbook編集・復元3回と1280×720のcapture 7枚を記録した。

8種WordArtの既定形状、3ページのbold/italic notes、notes/handout masterの読取り、modernコメント3件と返信3件のread-only COM読取りを確認した。日本語描画は確認済みだが埋込フォント消費は未証明。実modernコメントUI編集、field再計算、Office保存後の再取込み、任意文書・全描画の一致はこの検証に含まない。ヒストグラムは意図して除外し、資源ごとに既知XSDエラー2件の例外を維持する。

| 合成fixture | bytes | SHA-256 |
| --- | --- | --- |
| fresh.pptx | 3,359,310 | `ff26d652747f905ff95fc77755ab7d1c682d7a420c72ec3642ada3a30ef587d2` |
| body-edited.pptx | 3,359,324 | `b34a0c2fcd28ef75032d025bb0be38aa02e7fe8c6d546f3a9c31514a782d4798` |
| body-reedited.pptx | 3,359,325 | `e4b9f914f19c2c7c94162cd2809f00ca51e8688b5bdbf632bdf19d62110b5649` |

元fixtureのPK署名とSHAは確認前後で不変。これらは利用者の文書ではなく、ローカル検証専用の合成資料である。

## Known Remaining

- PDFの可視文字は編集可能なフォント文字ではない。96dpi直接印刷はダイアログ取消までで、実印刷・Office表示一致・PDF/UA/WCAG認証は未確認。
- グラフの完全3D・全Excel数値書式、任意WordArt調整、任意SVG/EMF/WMFは未対応。ヒストグラムのOffice用val属性は資源ごとに既知スキーマエラー2件を残す意図した例外。
- native付帯master削除・notesページresize・handout面付け・全locale日時評価は未対応。外部template2件は既存CFB／hash不一致で未変更・不採用、Officeフォント認識と任意PPTX互換は未確認。
- ローカルAIの日本語時制・意味保持、細い毛髪などのマスク精度は保証しない。重み利用条件の確認、人による適用レビュー、非中断CPU実行とRSS未測定の考慮が必要。
- modernコメントはoffline identityのみ、新規anchorはunknown限定でtext-rangeは不透明保持。実コメントUI編集は未確認。PII候補は手動確認用で削除機能ではない。回復は平文・既定OFFであり、今回のWindows実機・通常更新の合格を全面security監査や他OSの対応保証とはしない。

## 最終検証記録

固定版の完了結果を記録する。対象検証の重複件数や中断した試行は合算しない。`.artifacts/...` は公開対象外のローカル証拠名であり、公開リポジトリ内のダウンロードリンクではない。

| ゲート | 現在の状態 | 最終記録 |
| --- | --- | --- |
| Rust full core | PASS 511・FAIL 0・ignored 6、45集計、exit 0 | `.artifacts/completion-verified-core-IGlSqz/summary.json` |
| Node | PASS 90・FAIL 0・opt-in skip 1、実Qwen・全CJK fixtureを含む | `.artifacts/completion-node-final-4QUsM5/summary.json` と同所 `node.log`。installedは下記で別検証 |
| Studio build/lint | PASS。既存Fast Refresh警告3件とbundle chunk警告を維持 | `.artifacts/completion-native-20260919-6d914e/final-build-current`・同所 `lint`。setup buildも成功 |
| Studio Edge UI・生成 | 全202件PASS、skip/retry 0。生成は別4件PASS、合成model-response fixtureであり実モデル認定ではない | `.artifacts/eyedropper-final-aGkN4g/verification-summary.json` |
| native単体・実機 | 単体14件PASS。最新security修正後の実機6件PASS・skip 0 | `.artifacts/completion-native-20260919-6d914e/unit/command.log`、`.artifacts/bounded-native-20260919-8Cwlpf/native-full/execution.json` |
| setup build・分離installed | unsigned debug x64 GNU build成功。installed最終5/5 PASS・skip 0、169.6秒 | `.artifacts/bounded-native-20260919-8Cwlpf/setup-build/execution.json`、`.artifacts/installer-teardown-tkidbE/final-gates.json` と同所 `command.log` |
| Office | 上記3版の限定確認PASS、histogram例外は未解消 | `.artifacts/final-office-20260919-7e9c41/handoff.json` |
| 公開前確認 | 384 source/configのencoding不一致0。実装コミット178ファイル・index 3,945,996 bytesを検査し、作業ツリーとの一致・機密パターン・差分検査を確認 | `.artifacts/publication-final-4o8q1g/index-summary.json`。モデル・資料・私的パス・バイナリの混入なし。全面security／依存監査の認証ではない |
| 通常Windows更新 | 2026-09-19T07:02:09Z完了。NSIS exit 0、既存HKCU先・引数なしStart Menu起動・設定保持・installed native 4/4 PASS | `.artifacts/regular-desktop-update-20260919-1556-9e283c/final-proof.json`。[更新とhash](../testing/windows-setup.md#current-verified-update) |
| 通常commit/push | `murasamelabo/AISlide` の `main` へ通常push済み、実装SHA一致 | `54548846b1f651adc2ff070f73f122d63aaa77e2`。`.artifacts/publication-final-4o8q1g/feature-push.json`。本記録は後続の文書専用コミットで追加し、hosted CIの成功はローカル合格から推定しない |

実機6件の通常ビルドは1,015,062,016 bytes、SHA-256 `7792a18c7524f428ff51687d918f17b287c69289da4da603a29d814f39520c00`。setup元バイナリおよびインストール済みバイナリとは別成果物として記録する。分離installed試験の300秒は約1GBのdebug payloadを含む試験ライフサイクル専用で、製品のtimeout変更ではない。終了同期の修正と過去の失敗の因果限界は[setup記録](../testing/windows-setup.md#current-verified-update)を参照。

## 公開と更新

- [x] 限定範囲の実装と5件のレビュー修正を統合し、製品コードを固定する。
- [x] 最終コードで対象テスト、Rust workspace、Node、Studio build/lint、ブラウザー、ネイティブ、必要な限定Office確認を完了し、上表へ結果を記録する。
- [x] 並行編集文書を含む最終文字コード、機密パターン、公開対象、元ファイル非変更、追加した主要依存のライセンス表記を確認した。生成資料・モデル重みは公開しない。
- [x] 許可済みの範囲を既存Publicリポジトリ `murasamelabo/AISlide` の `main` へ通常commit/pushし、実装SHAとGitHub上の一致を記録した。force push・visibility変更・ライセンス選択・リリース公開は行わない。
- [x] 稼働中アプリと未保存作業を保護し、通常のユーザー単位セットアップでデスクトップを更新する。
- [x] インストール先バイナリ、引数なしStart Menu起動、通常プロファイルで表示・応答するウィンドウを確認し開いたまま残す。設定・回復ストアの内容保持はinstallと分離試験後、通常起動前まで確認済み。通常起動後の正当なプロファイル更新まで不変とはしない。

既存の444/66/175件は開始時の基準であり、この追加開発後の検証結果ではない。