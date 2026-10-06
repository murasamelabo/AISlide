# 再利用可能な資料作成スキル

用途の選択、日本語の用途別専門構成、日英の編集、AISlide の PPTX 実行を分離した、リポジトリ管理の11スキルです。特定の業界、顧客、個人環境に依存しません。このページの導入手順は利用者向けの説明であり、スキル配置、MCP 設定、公開を自動実行するものではありません。

## 使い分け

| スキル | 担当 | 入口 |
| --- | --- | --- |
| [slide-planning](slide-planning/SKILL.md) | 6系統・33用途を選び、専門スキル・理由・担当ページを提示して同じ台帳をつなぐ | 新規構成・明示された再構成・担当候補の相談。候補提示だけなら委譲しない |
| [japanese-editing](japanese-editing/SKILL.md) | 意味を保った日本語の推敲。プレゼンの見出し・本文・図表ラベル・ノート、提案、報告、メール、技術文書など | 日本語を読みやすくする依頼。文章のみでも単独使用可能 |
| [english-editing](english-editing/SKILL.md) | 英語の語順・冠詞・法助動詞・修飾・並列性を踏まえた推敲。プレゼン、提案、報告、メールなど | 英文を明快で自然にする依頼。日本語ルールの翻訳ではない独立した編集手順 |
| [organization-deck-ja](organization-deck-ja/SKILL.md) | 組織・人材の読者との適合、実態、根拠と図表 | 日本語の会社紹介、採用、文化、入社案内、ポートフォリオ |
| [commercial-deck-ja](commercial-deck-ja/SKILL.md) | 顧客の判断、比較条件、導入負担、実画面と成果 | 日本語のサービス紹介、営業提案、製品デモ、導入事例、協業提案 |
| [management-deck-ja](management-deck-ja/SKILL.md) | 数値の整合、資源配分、選択肢と承認範囲 | 日本語の資金調達、事業計画、成長戦略、決算、稟議、サステナビリティ |
| [delivery-deck-ja](delivery-deck-ja/SKILL.md) | 計画と実績、影響、分担、期限、支援・合意 | 日本語のキックオフ、進捗、振り返り、制度変更、全社会議 |
| [learning-deck-ja](learning-deck-ja/SKILL.md) | 到達目標、作業成果、方法と証拠、適用限界 | 日本語の研修、操作手順、討議、研究発表、調査報告。技術解説は別担当 |
| [public-deck-ja](public-deck-ja/SKILL.md) | 視点、立場別の影響、発表条件、表現案の判断 | 日本語の講演、製品発表、住民説明、政策提言、障害説明、創作提案 |
| [tech-deck-ja](tech-deck-ja/SKILL.md) | 日本語の技術ストーリー、根拠台帳、説明順序、報告調の本文 | 日本語の技術説明資料を新規に構成するとき |
| [aislide-authoring](aislide-authoring/SKILL.md) | AISlide core / MCP / SDK による編集可能な PPTX の作成、検証、納品 | PPTX の編集・書き出し、または構成後の実行 |

`slide-planning` は全体構成と担当選択、7つの専門スキルは日本語の用途別の構成判断、日英編集は文言、`aislide-authoring` は PPTX 実行を担当します。専門担当は読者・根拠・図表・ノートを同じ台帳へ返し、委譲中に他担当を再呼び出ししません。同じ専門スキルは一度だけ使い、補助用途は指定ページだけに適用します。直接呼び出しで用途が合わない場合は `slide-planning` へ一度渡し、そこから呼ばれていた場合は不一致を依頼元へ返します。

専門スキルの提示は実行の許可ではありません。候補だけを求められた場合は名前・理由・担当ページ案までで止めます。構成の依頼があれば担当を示してから委譲します。混在資料は文章・ページごとに言語を選び、日本語部分だけに専門スキルを適用します。英語部分は用途別参照と `english-editing` を使い、翻訳や日本語の報告調を強制しません。承認済みの構成は、再構成の依頼がなければ保持します。

[用途カタログ](slide-planning/references/catalog.md) は組織・人材、営業・協業、経営・投資、業務・推進、学習・探究、広報・公共・創作の6系統です。主用途は1つ、補助用途は必要なときだけ1つまで選び、対応する節だけを参照します。言語、投影・配布・スマホ閲覧、業種、見た目は用途と別の条件です。これは編集・構成の指針であり、決定論的な分類器や新しいMCPパラメーターではありません。

非技術の32用途には、判断と境界、入力不足時の対応、構成の分岐、ページとノートの架空台帳、部品の適用・回避条件、失敗例と修正、完了基準を用意しています。例えば [営業提案](slide-planning/references/commercial.md#sales-proposal)、[採用](slide-planning/references/organization.md#recruiting-pitch)、[研修](slide-planning/references/learning.md#training) を参照できます。専門スキルは選択した用途の `##` 見出しから次の同レベル見出しまでを読み、系統全体を毎回読み込みません。例の枚数・F1などの根拠ID・仮定を、利用者の資料へそのまま流用しません。

静的テストは項目・参照・部品IDの整合を確認します。モデル出力は [固定比較ケース](slide-planning/references/checks-and-sources.md) で別に評価し、ガイドの詳しさや文字数を品質改善の証明としません。実際の表示確認・読者の理解・モデルによる自動選択も別の評価です。

日本語編集では [nanaism/yomiyasu](https://github.com/nanaism/yomiyasu)（MIT）の意味保持・文脈に基づく推敲を参考に、独自の手順と架空例を記述しています。上流のスキル本文・文章例・検査スクリプトは複製・インストールしていません。箇条書き比率や文長を固定の合否基準にせず、投影・配布・ノートの役割に合わせます。これは AISlide 全体のライセンス選定ではありません。

英語編集では、NARA の plain language 原則、Google の条件と指示の配置、assertion-evidence のプレゼン方針を参照し、Humanizer と既存の執筆スキルを比較しました。[調査元と意味保持の確認例](english-editing/references/checks-and-sources.md) に採用点と採用しなかった規則を記載しています。読み手に不要な推測をさせないことを目標にし、受動態・代名詞・長文・特定の単語を一律に禁止しません。AI検出器の回避や筆者の体験の創作は目的に含めません。

## 前提条件

- スキルに対応したエージェントホストを利用します。下記の配置先は VS Code / GitHub Copilot の規約です。他のホストでは対応するスキル探索先を確認してください。
- AISlide MCP の接続は別の前提条件です。ソースからの準備は [実行手順](../../README.md#run) と [MCP 手順](../../README.md#mcp) を参照してください。スキルのコピーだけで Node.js、Rust、core、MCP、フォントはインストールされません。
- 日本語グリフを持つフォントを実行環境で確認し、作成時に選びます。フォント名の指定は導入・ライセンス確認・埋め込みを代行しません。
- `<repo>` はこのソースのチェックアウト、`<project>` は資料作成先リポジトリ、`<output>` は承認する新規出力用ディレクトリ、`<assets>` は承認する入力素材ディレクトリです。すべて利用者が選ぶプレースホルダーで、そのまま実行しません。`~` はホームディレクトリを表します。

MCP 起動指定の例です。利用者のホスト設定で実際の絶対パスに置き換え、書き込み先と素材の読み取り範囲を確認します。

```sh
node "<repo>/tools/mcp.mjs" --output-dir "<output>" --asset-dir "<assets>"
```

`--asset-dir` は起動時だけの権限設定です。画像登録リクエストからルートを増やせません。変更のために再接続する場合は、プロセス内の文書・画像ハンドルを失う前に作業を保存してください。スキルを置くために既存接続を停止する必要はありません。

## 配置先を一つ選ぶ

文章編集だけなら対象言語の編集スキルを単独で使えます。新規構成には `slide-planning`、日本語なら必要な用途の専門スキルと `japanese-editing`、PPTX 作成には `aislide-authoring` を加え、**同じスコープへ、名前を変えずディレクトリごと** コピーします。配置しても全スキルを毎回読み込みません。

新しい6専門スキルは `slide-planning/references/` の用途別指針を参照します。専門スキルを直接使う場合も `slide-planning/` を参照ファイルごと配置してください。日本語の全用途を扱うなら7専門担当すべて、英語編集も含めた一式なら11スキルを配置します。専門担当が未配置でも、`slide-planning` は不足を明示して対応する参照節で構成できます。自動インストールはしません。

| コピー元 | ユーザースコープ: 複数リポジトリで使用 | リポジトリスコープ: そのプロジェクトだけで使用 |
| --- | --- | --- |
| `<repo>/.github/skills/slide-planning/` | `~/.copilot/skills/slide-planning/` | `<project>/.github/skills/slide-planning/` |
| `<repo>/.github/skills/japanese-editing/` | `~/.copilot/skills/japanese-editing/` | `<project>/.github/skills/japanese-editing/` |
| `<repo>/.github/skills/english-editing/` | `~/.copilot/skills/english-editing/` | `<project>/.github/skills/english-editing/` |
| `<repo>/.github/skills/aislide-authoring/` | `~/.copilot/skills/aislide-authoring/` | `<project>/.github/skills/aislide-authoring/` |
| `<repo>/.github/skills/tech-deck-ja/` | `~/.copilot/skills/tech-deck-ja/` | `<project>/.github/skills/tech-deck-ja/` |
| `<repo>/.github/skills/organization-deck-ja/` | `~/.copilot/skills/organization-deck-ja/` | `<project>/.github/skills/organization-deck-ja/` |
| `<repo>/.github/skills/commercial-deck-ja/` | `~/.copilot/skills/commercial-deck-ja/` | `<project>/.github/skills/commercial-deck-ja/` |
| `<repo>/.github/skills/management-deck-ja/` | `~/.copilot/skills/management-deck-ja/` | `<project>/.github/skills/management-deck-ja/` |
| `<repo>/.github/skills/delivery-deck-ja/` | `~/.copilot/skills/delivery-deck-ja/` | `<project>/.github/skills/delivery-deck-ja/` |
| `<repo>/.github/skills/learning-deck-ja/` | `~/.copilot/skills/learning-deck-ja/` | `<project>/.github/skills/learning-deck-ja/` |
| `<repo>/.github/skills/public-deck-ja/` | `~/.copilot/skills/public-deck-ja/` | `<project>/.github/skills/public-deck-ja/` |

Windows の `~` は通常 `%USERPROFILE%` に相当します。ホストが対応する `~/.agents/skills/` 等を既に使っている場合は、その一つの探索先へ統一します。すべての探索先へ複製しません。この AISlide リポジトリ内では11スキルともリポジトリスコープにあるため、追加コピーは不要です。

1. ユーザー用かリポジトリ用かを選び、ホストが読む他の探索先も含めて同名スキルの有無を確認します。既存の個人版・マーケットプレイス版を無断で上書き・削除しません。
2. コピー先に同名のディレクトリがなければ親ディレクトリを用意し、必要な役割のスキルをコピーします。既存版があれば差分を確認し、どちらを有効にするか利用者が選びます。同名の複数版の読み込み優先順位に依存しません。
3. `SKILL.md` の直上のディレクトリ名と frontmatter の `name` が一致することを確認します。隣のスキルと `references/` への相対参照を維持します。ファイルは UTF-8 BOM 一つを保ちます。
4. エージェントホストのスキル一覧・診断、または明示呼び出しで、各名前が意図した一つの配置先から認識されることを確認します。スキルを認識できることと MCP に接続できることは別々に確認します。

役割を分けて読み込むこと自体は重複ではありません。構成、文言の編集、PPTX 実行を同じ台帳でつなぎます。編集担当は依頼元へ結果を返し、別々の調査やスライド生成、相互転送のループを始めません。既存の常時適用ルールに矛盾がある場合は、利用者が解消してください。

コピー後の基本手順はスキル内で完結します。詳細な API 文書はコピー先からの固定相対リンクにせず、必要なときに `<repo>` のチェックアウトで確認します。上表の配置は公開・配布の許可を意味しません。プロジェクトのライセンスは未選定のため、再配布時は [ライセンスの状態](../../README.md#license) を確認してください。

## 明示的な呼び出し例

自動認識だけに頼らず、最初の依頼で構成と実行の担当を明記できます。以下はすべて架空の依頼例です。

### 用途から新規構成する

```text
slide-planning で架空企業の採用資料を8枚に構成してください。
読者はエンジニア候補者。仕事と期待を理解して応募を検討できることが目的です。
主用途、専門スキル名、選択理由、担当ページ案を先に示してください。
その後、適合する専門スキルに一度だけ委譲し、同じ台帳へ構成をまとめてください。
日本語、10分の説明と配布PDF向け。待遇や実績は創作しないでください。
今回は構成と文言までで、PPTXの作成・公開はしないでください。
```

英語なら「Use slide-planning for a sales proposal in UK English. Include two customer-case slides as supporting evidence. Keep the supplied figures and caveats. Return an outline only.」のように、目的と言語・実行範囲を分けて指定します。

### 担当候補の提示だけ

```text
slide-planningで、日本語の架空サービス導入提案に合う専門スキルを提示してください。
読者は顧客の決裁者で、比較・費用・導入負担から採否を判断することが目的です。
予定は8枚。主用途、専門スキル名、選択理由、担当ページ案だけを示してください。
今回は候補の提示だけで、専門スキルへの委譲・構成・PPTX作成はしないでください。
```

この例は `sales-proposal` と `commercial-deck-ja` が候補です。技術製品でも題材だけで `tech-deck-ja` にしません。選択結果の `specialist_selections` は台帳用の記録で、MCP引数や guided `profile_id` ではありません。

### 専門スキルを直接使う

```text
management-deck-jaで、添付した架空の投資計画を日本語の稟議資料8枚に構成してください。
主用途はdecision-proposal。決裁者が承認範囲と条件を判断できる資料にします。
承認対象、見送る案を含む選択肢、費用、リスクを、根拠IDと図表案付きで示してください。
不足情報は未確認とし、構成と文言までに留めてください。PPTX作成・公開は不要です。
```

### 技術以外のプレゼン・一般文書

```text
japanese-editing で、この架空の業務改善提案の日本語を整えてください。
読者は現場責任者、用途は5分の説明とPDF配布です。
見出し・本文・図表ラベル・ノートをページIDごとに示してください。
提案と決定事項を混同せず、数値・母数・対象条件・未確認事項は残してください。
今回は文言の編集だけで、PPTXの変更や公開は行わないでください。
```

メールや報告なら「japanese-editing で、次のメールを簡潔に。依頼の強さと期限は変えないでください」のように、用途と保持したい条件を指定します。

### 英語のプレゼン・一般文書

```text
Use english-editing to revise this synthetic proposal for an international
management audience. Use UK English. The deck supports a five-minute talk
and must also make sense as a PDF without speaker notes.
Keep the proposal status, figures, scope and uncertainty unchanged.
Return headline, body, chart labels and speaker notes by slide ID.
Do not change the PPTX or publish anything yet.
```

英語のメールなら「Use english-editing to make this request clearer. Keep the deadline and level of politeness unchanged.」のように指定できます。原文だけで `may not` の意味や日付の形式を判断できなければ、勝手に確定させず確認事項を返します。

### 新規の日本語技術資料

```text
tech-deck-ja で日本語技術資料を構成し、aislide-authoring で
AISlide MCP の編集可能な PPTX として新規作成してください。
題材は「同期処理とキューによる非同期処理の比較」という架空例です。
対象は技術レビュー参加者、全 6 枚、日本語の報告調とします。
特定の製品・組織を前提にせず、実測値や顧客事例を創作しないでください。
根拠がない項目は仮定・未測定と明記し、構成と根拠台帳を引き継いでください。
作成時に利用可能な日本語フォントを確認し、タイトル位置、マスター、
ヘッダー、フッター、本文領域を揃えて再利用してください。
図は管理対象 graphs / parts を優先し、各ページを確認してから
承認済み出力先の新しい名前へ保存してください。既存資料は変更しません。
```

### 既存 PPTX の限定編集

```text
aislide-authoring を使い、指定した PPTX の対象ページだけを修正してください。
日本語の構成は承認済みなので tech-deck-ja で再構成しないでください。
原本を上書きせず、指定した画像とノートを保持して別名へ書き出してください。
変更ページと確認範囲、残った診断を報告してください。
```

### 構成のみ

```text
tech-deck-ja で日本語の技術レビュー資料の構成案と根拠台帳だけを作ってください。
PPTX の作成・MCP の変更・公開はまだ行わないでください。
```

## 架空例のスライド台帳

上の 6 枚の依頼に対応する説明用データです。実在のシステム、性能測定、導入推奨を示しません。以下の `S1` は全ページ共通の仮定 ID で、実資料の出典を装うものではありません。

| ID | 日本語タイトル | 要点と表現 | 根拠・ノート |
| --- | --- | --- | --- |
| s1 | 同期・非同期の選択条件 | 問いと検討範囲。本文に「架空例・未測定」 | S1: 依頼を受ける処理と実処理を持つ架空構成 |
| s2 | 比較の前提 | 応答の意味、完了条件、失敗時の扱いを共通軸にする | S1: 実際の要件は未確定 |
| s3 | 同期処理の流れ | 呼び出し元、処理、応答を管理対象 graph で表す | S1: 処理完了後に応答する仮定 |
| s4 | キューを介した処理の流れ | 呼び出し元、受付、キュー、実行担当と関係ラベル | S1: 受付と完了を区別。性能改善は主張しない |
| s5 | 選択時に確認する条件 | 同じ評価軸の表または対応する part | S1: 再試行、順序、重複、監視の要件は未検証 |
| s6 | 判断前に必要な検証 | 計測項目と未確認事項。結論を捏造しない | S1: 待ち時間・処理時間・失敗率は未測定 |

実資料では S1 を出典名、ページ・節・表セル等の位置、取得日、バージョン、適用条件を持つ根拠 ID に置き換え、仮定は仮定として残します。出典画像が必須なら利用者が承認したローカル原図を登録し、再描画で代用しません。画像のバイナリや base64 は会話へ出力しません。

作成時の `setup`、余白、単一入力の rich text、画像一括登録の条件は [日本語技術資料の実行手順](../../docs/authoring/README.md#japanese-technical-decks) を参照してください。公開スキーマにない操作や自動階層レイアウトを前提にせず、`discover_tools` / `get_tool_schema` で接続先の機能を確認します。