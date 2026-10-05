# 選択の確認例と調査元

以下は架空の依頼による手動確認ケース。文字列の契約テストを、モデルの分類・編集品質の評価と同一視しない。
同じ入力・モデル・設定を記録し、選択理由、参照した節、台帳、意味保持、余計な操作の有無を個別に評価する。

| 入力の要点 | 期待する選択・動作 | 避ける動作 |
| --- | --- | --- |
| AI企業のエンジニア採用。候補者に仕事と期待を伝える | recruiting-pitch。業界や職種は補足条件 | 技術用語だけで tech-deck-ja を起動 |
| 会社紹介として渡されたが、社員の判断・行動を揃えたい | culture-deck。題名より到達点を優先 | 全社紹介・採用・文化を全部並べる |
| webinar で操作を練習し、各自が作業を完了する | procedure または training。操作完遂か概念習得かで判断 | webinar を用途として固定 |
| 投資家に追加面談を依頼する pitch deck | fundraising-pitch。事業性と検証実績 | sales-proposal と混同して導入契約を求める |
| 顧客向け sales-proposal に導入事例を2枚含める | 主用途を維持。必要なら secondary_purpose=customer-case を2枚に限定 | 導入事例の全体構成を追加 |
| 承認済みの8枚を wording-only で自然な英語に | english-editing のみ。構成・枚数を保持 | slide-planning を起動して再構成 |
| 日本語の技術解説を新規構成。実行担当から構成だけ依頼 | 同じ台帳で技術専門担当の結果を統合し、依頼元に返す | 相互転送、再調査、別のPPTX作成 |
| UK English の決算説明。投影にも配布PDFにも使う | financial-results。媒体を別に記録し、留保を本文に残す | 日本語の報告調や注記のノート限定化 |
| 職場の制度変更案を説明し、意見を集めたい | change-announcement。案と決定を明確に区別 | 決定済みの義務として編集 |
| 講義を含む討議会で、参加者が最後に改善案を作る | workshop が主用途。説明は作業の材料 | 参加者の回答や合意をあらかじめ作る |
| カタログにない式典用の記念スライド | 適合不足を明示し、目的に合う個別案を提案 | 近いIDへ無理に分類、勝手にスキルを追加 |
| PPTXの2ページ目の誤字だけ修正 | aislide-authoring と対象言語の編集。対象外は保持 | 用途の再選定、全ページの作り直し |

参照欠落時は既存の意味保持・同一台帳・作業範囲を維持し、不足を明示する。外部スキルやフォントを自動導入しない。

調査日: 2026-10-04。分類は以下を参考にした独自の整理であり、資料市場の網羅的・統計的分類ではない。
リンク先は根拠の確認用。参照するたびに再調査・ダウンロードする手順ではない。

| 調査元 | 採用した観点と限界 |
| --- | --- |
| [Slideland 資料種別](https://www.slideland.tech/docs/material) / [会社紹介・採用・文化](https://www.slideland.tech/docs/material/company-introduction) / [分類一覧](https://www.slideland.tech/docs/category-list) | 用途・ページ役割・業種・見た目を分離。会社紹介の一分類から目的の異なる3用途を分けた。掲載作品やデザインは複製しない。 |
| [Netflix Culture](https://jobs.netflix.com/culture) | 価値観と具体行動、適合の説明。特定企業の価値観を他社へ移植せず、現在のメモ形式をスライドの必須構成にしない。 |
| [Sequoia: Writing a Business Plan](https://sequoiacap.com/article/writing-a-business-plan/) | 事業目的、課題、なぜ今か、事業性と投資家の判断。章の固定や架空の市場・財務数値は採用しない。 |
| [トヨタ 決算報告](https://global.toyota/jp/ir/financial-results/) | 説明資料、スクリプト付き資料、質疑の役割分担。個社の数値・構成をテンプレート化しない。 |
| [Atlassian Project Kickoff](https://www.atlassian.com/team-playbook/plays/project-kickoff) | 目的、役割、成功条件の合意。会議時間や人数の例を一般的な固定値にしない。 |
| [CMU Lectures](https://www.cmu.edu/teaching/designteach/teach/instructionalstrategies/lectures.html) | 学習目標、例、練習、要約、理解確認。研修を結論だけの資料にしない。 |
| [MIT Slideshow](https://mitcommlab.mit.edu/broad/commkit/slideshow/) / [Policy Presentation](https://mitcommlab.mit.edu/broad/commkit/policy-presentation/) | 読者と発表目的による根拠・説明順序の違い。重要な限界や反証の省略、全用途への同じ見出し・文字量の強制はしない。 |

既存の tech-deck-ja、日英編集を再利用する。ローカルの frontend-slides はHTML実行、investor-materials は投資家資料、brand-voice は語り口の抽出が中心で、33用途の選択と共通台帳を統括する役割とは異なる。
外部スキル本文・例・スクリプト・画像は同梱していない。CMU/MIT 等の出典の利用条件を別途尊重し、プロジェクト全体のライセンスを選定するものではない。