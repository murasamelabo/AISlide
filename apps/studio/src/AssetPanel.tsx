import { useRef, useState } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { Database, Server, Cloud, Globe, Router, Monitor, Smartphone, Folder, FileText, Mail, User, Users, ShieldCheck, LockKeyhole, KeyRound, Check, CircleAlert, Search, Settings, Workflow, Network, GitBranch, ArrowRight, ArrowLeftRight, Calendar, Clock, ChartNoAxesCombined, Building2, Briefcase, Layers, Link, MessageSquare, Upload, Download, HardDrive, Bell, ClipboardPaste } from 'lucide-react'
import {
  icons as lucideIcons, ChevronLeft, ChevronRight,
  Cpu, MemoryStick, CircuitBoard, Bot, BrainCircuit, Sparkles, Terminal, CodeXml, Braces, Container, Blocks, GitPullRequest,
  Wifi, EthernetPort, RadioTower, Cable, Satellite, CloudUpload, CloudDownload, Plug,
  Fingerprint, ScanFace, ShieldAlert, Eye, ScanLine, ChartColumn, ChartPie, TrendingUp, Gauge, Target,
  Lightbulb, Rocket, Flag, ListChecks, Kanban, ClipboardCheck, Handshake, Presentation,
  CreditCard, Wallet, Receipt, ShoppingCart, Store, Factory, Package, Truck, Warehouse, MapPin, Route, Plane, Ship, TrainFront,
  GraduationCap, BookOpen, FlaskConical, Microscope, Stethoscope, HeartPulse, Leaf, Recycle,
} from 'lucide-react'
import type { AssetInput } from './types'
import { fileAssets, svgAsset } from './api'
import { Tool } from './Tool'
import categoryData from './lucide-categories.json'

const metadata: { categories: Record<string, string>; icons: Record<string, { categories: string[]; tags: string[] }> } = categoryData

const featuredIcons = [
  { name: 'Database', icon: Database, tags: 'data storage データベース 保存' }, { name: 'Server', icon: Server, tags: 'compute サーバー' },
  { name: 'Cloud', icon: Cloud, tags: 'クラウド' }, { name: 'Globe', icon: Globe, tags: 'internet web インターネット' },
  { name: 'Router', icon: Router, tags: 'network ルーター' }, { name: 'Monitor', icon: Monitor, tags: 'computer pc パソコン' },
  { name: 'Smartphone', icon: Smartphone, tags: 'mobile phone スマートフォン' }, { name: 'Folder', icon: Folder, tags: 'files フォルダー' },
  { name: 'Document', icon: FileText, tags: 'file paper 文書' }, { name: 'Mail', icon: Mail, tags: 'email メール' },
  { name: 'User', icon: User, tags: 'person 人 利用者' }, { name: 'Users', icon: Users, tags: 'team people チーム' },
  { name: 'Shield', icon: ShieldCheck, tags: 'security セキュリティ' }, { name: 'Lock', icon: LockKeyhole, tags: 'privacy 鍵 保護' },
  { name: 'Key', icon: KeyRound, tags: 'authentication 認証' }, { name: 'Check', icon: Check, tags: 'done complete 完了' },
  { name: 'Alert', icon: CircleAlert, tags: 'warning error 警告' }, { name: 'Search', icon: Search, tags: '検索' },
  { name: 'Settings', icon: Settings, tags: 'configuration 設定' }, { name: 'Workflow', icon: Workflow, tags: 'process フロー' },
  { name: 'Network', icon: Network, tags: 'topology ネットワーク' }, { name: 'Branch', icon: GitBranch, tags: 'route 分岐' },
  { name: 'Arrow', icon: ArrowRight, tags: 'right 矢印' }, { name: 'Exchange', icon: ArrowLeftRight, tags: 'sync exchange 同期' },
  { name: 'Calendar', icon: Calendar, tags: 'schedule カレンダー' }, { name: 'Clock', icon: Clock, tags: 'time 時間' },
  { name: 'Chart', icon: ChartNoAxesCombined, tags: 'graph metrics グラフ' }, { name: 'Building', icon: Building2, tags: 'office company 会社' },
  { name: 'Briefcase', icon: Briefcase, tags: 'work 業務' }, { name: 'Layers', icon: Layers, tags: 'stack レイヤー' },
  { name: 'Link', icon: Link, tags: 'connect 接続' }, { name: 'Message', icon: MessageSquare, tags: 'chat メッセージ' },
  { name: 'Upload', icon: Upload, tags: 'アップロード' }, { name: 'Download', icon: Download, tags: 'ダウンロード' },
  { name: 'Drive', icon: HardDrive, tags: 'disk storage ストレージ' }, { name: 'Bell', icon: Bell, tags: 'notification 通知' },
  { name: 'CPU', icon: Cpu, tags: 'processor compute chip プロセッサ 演算 半導体' }, { name: 'Memory', icon: MemoryStick, tags: 'ram hardware メモリ ハードウェア' },
  { name: 'Circuit', icon: CircuitBoard, tags: 'electronics board iot 回路 基板 電子' }, { name: 'Bot', icon: Bot, tags: 'ai agent robot automation ロボット エージェント 自動化' },
  { name: 'AI brain', icon: BrainCircuit, tags: 'artificial intelligence machine learning model 人工知能 機械学習 モデル' }, { name: 'Sparkles', icon: Sparkles, tags: 'ai generate magic 生成 キラキラ' },
  { name: 'Terminal', icon: Terminal, tags: 'cli console shell ターミナル コマンド' }, { name: 'Code', icon: CodeXml, tags: 'developer source html xml 開発 ソース コード' },
  { name: 'Braces', icon: Braces, tags: 'api json data object programming オブジェクト プログラミング' }, { name: 'Container', icon: Container, tags: 'deploy virtual infrastructure コンテナ 配備 仮想化' },
  { name: 'Blocks', icon: Blocks, tags: 'module component architecture モジュール 部品 構成' }, { name: 'Pull request', icon: GitPullRequest, tags: 'git code review merge 開発 レビュー マージ' },
  { name: 'Wi-Fi', icon: Wifi, tags: 'wireless network internet 無線 ワイヤレス 通信' }, { name: 'Ethernet', icon: EthernetPort, tags: 'lan wired network port 有線 ポート' },
  { name: 'Radio tower', icon: RadioTower, tags: 'antenna broadcast signal アンテナ 基地局 放送' }, { name: 'Cable', icon: Cable, tags: 'wire connect usb ケーブル 配線 接続' },
  { name: 'Satellite', icon: Satellite, tags: 'space orbit communication 宇宙 衛星 通信' }, { name: 'Cloud upload', icon: CloudUpload, tags: 'backup storage sync クラウド バックアップ' },
  { name: 'Cloud download', icon: CloudDownload, tags: 'restore storage sync クラウド 復元 同期' }, { name: 'Plug', icon: Plug, tags: 'power connect electricity 電源 電気 接続' },
  { name: 'Fingerprint', icon: Fingerprint, tags: 'identity biometric authentication 指紋 生体認証 本人確認' }, { name: 'Face scan', icon: ScanFace, tags: 'identity biometric authentication 顔認証 顔スキャン' },
  { name: 'Security alert', icon: ShieldAlert, tags: 'risk threat warning セキュリティ 脅威 リスク' }, { name: 'Eye', icon: Eye, tags: 'visibility watch monitoring 可視性 監視 表示' },
  { name: 'Scan', icon: ScanLine, tags: 'ocr barcode reader スキャン 読み取り' }, { name: 'Bar chart', icon: ChartColumn, tags: 'analytics metrics graph 分析 棒グラフ 集計' },
  { name: 'Pie chart', icon: ChartPie, tags: 'analytics share ratio 円グラフ 比率 構成比' }, { name: 'Trend', icon: TrendingUp, tags: 'growth forecast analytics 成長 予測 傾向' },
  { name: 'Gauge', icon: Gauge, tags: 'dashboard performance speed ダッシュボード 性能 速度' }, { name: 'Target', icon: Target, tags: 'goal objective kpi 目標 目的 指標' },
  { name: 'Idea', icon: Lightbulb, tags: 'insight innovation lightbulb アイデア 発想 電球' }, { name: 'Rocket', icon: Rocket, tags: 'launch startup release ローンチ 起業 リリース' },
  { name: 'Flag', icon: Flag, tags: 'milestone checkpoint finish 旗 マイルストーン 達成' }, { name: 'Checklist', icon: ListChecks, tags: 'tasks todo requirements タスク 要件 チェックリスト' },
  { name: 'Kanban', icon: Kanban, tags: 'project workflow board カンバン プロジェクト 進捗' }, { name: 'Approval', icon: ClipboardCheck, tags: 'audit check validation 承認 監査 検証' },
  { name: 'Handshake', icon: Handshake, tags: 'partner agreement collaboration 提携 契約 協業' }, { name: 'Presentation', icon: Presentation, tags: 'slides meeting seminar 発表 プレゼン 会議' },
  { name: 'Credit card', icon: CreditCard, tags: 'payment billing purchase 決済 支払い クレジットカード' }, { name: 'Wallet', icon: Wallet, tags: 'budget money finance 財布 予算 財務' },
  { name: 'Receipt', icon: Receipt, tags: 'invoice expense billing 領収書 経費 請求' }, { name: 'Cart', icon: ShoppingCart, tags: 'shopping commerce order カート 買い物 注文' },
  { name: 'Store', icon: Store, tags: 'shop retail commerce 店舗 小売 販売' }, { name: 'Factory', icon: Factory, tags: 'industry manufacturing production 工場 製造 生産' },
  { name: 'Package', icon: Package, tags: 'box product shipment 荷物 商品 梱包' }, { name: 'Truck', icon: Truck, tags: 'delivery logistics freight 配送 物流 トラック' },
  { name: 'Warehouse', icon: Warehouse, tags: 'inventory stock distribution 倉庫 在庫 保管' }, { name: 'Map pin', icon: MapPin, tags: 'location place address 場所 位置 拠点' },
  { name: 'Route', icon: Route, tags: 'journey path navigation 経路 ルート 導線' }, { name: 'Plane', icon: Plane, tags: 'flight travel airport 飛行機 航空 旅行' },
  { name: 'Ship', icon: Ship, tags: 'sea cargo transport 船 海運 輸送' }, { name: 'Train', icon: TrainFront, tags: 'rail transit transport 電車 鉄道 交通' },
  { name: 'Graduation cap', icon: GraduationCap, tags: 'education learning school 教育 学習 学校 卒業' }, { name: 'Book', icon: BookOpen, tags: 'knowledge documentation reading 本 知識 教材' },
  { name: 'Lab', icon: FlaskConical, tags: 'science chemistry experiment 科学 化学 実験' }, { name: 'Microscope', icon: Microscope, tags: 'research biology analysis 顕微鏡 研究 生物' },
  { name: 'Stethoscope', icon: Stethoscope, tags: 'medical doctor clinic 医療 医師 診察' }, { name: 'Health', icon: HeartPulse, tags: 'healthcare wellness heartbeat 健康 心拍 ヘルスケア' },
  { name: 'Leaf', icon: Leaf, tags: 'environment sustainability nature 環境 自然 脱炭素' }, { name: 'Recycle', icon: Recycle, tags: 'reuse circular sustainability リサイクル 再利用 循環' },
]

const featured = new Map(featuredIcons.map((entry, order) => [entry.icon, { ...entry, order }] as const))
const featuredNames = new Set(featuredIcons.map((entry) => entry.name))
const icons = Object.entries(lucideIcons).map(([id, icon]) => {
  const preferred = featured.get(icon)
  const name = id.replace(/([a-z0-9])([A-Z])/g, '$1 $2').replace(/([A-Z])([A-Z][a-z])/g, '$1 $2')
  const details = metadata.icons[id]
  return { id, icon, name: preferred?.name ?? (featuredNames.has(name) ? `${name} (plain)` : name), tags: `${preferred?.tags ?? ''} ${details.tags.join(' ')}`, categories: details.categories, order: preferred?.order ?? featuredIcons.length }
}).sort((left, right) => left.order - right.order)
const iconsPerPage = 60
const categories = Object.entries(metadata.categories).map(([id, name]) => ({ id, name, count: icons.filter((icon) => icon.categories.includes(id)).length })).sort((left, right) => left.name.localeCompare(right.name, 'en'))

export function AssetPanel({ onInsert, onBusy, maxFiles = 8, showSize = true }: { onInsert: (assets: AssetInput[]) => Promise<void>; onBusy: (busy: boolean) => void; maxFiles?: number; showSize?: boolean }) {
  const [query, setQuery] = useState('')
  const [category, setCategory] = useState('all')
  const [iconPage, setIconPage] = useState(0)
  const [selected, setSelected] = useState(icons[0])
  const [color, setColor] = useState('#0017c1')
  const [size, setSize] = useState(showSize ? 96 : 48)
  const [stroke, setStroke] = useState(2)
  const [mode, setMode] = useState<'library' | 'svg'>('library')
  const [svg, setSvg] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const input = useRef<HTMLInputElement>(null)
  const iconGrid = useRef<HTMLDivElement>(null)
  const gate = useRef(false)
  function goToIconPage(value: number) {
    setIconPage(value)
    iconGrid.current?.scrollTo({ top: 0 })
  }
  async function insert(action: () => Promise<AssetInput[]>) {
    if (gate.current) return
    gate.current = true; setBusy(true); onBusy(true); setError('')
    try { const assets = await action(); if (assets.length > maxFiles) throw new Error(`Select at most ${maxFiles} asset${maxFiles === 1 ? '' : 's'}`); await onInsert(assets) }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { gate.current = false; setBusy(false); onBusy(false) }
  }
  const terms = query.trim().toLowerCase().split(/[\s_-]+/).filter(Boolean)
  const matches = icons.filter((entry) => (category === 'all' || entry.categories.includes(category)) && terms.every((term) => `${entry.name} ${entry.id} ${entry.tags}`.toLowerCase().includes(term)))
  const offset = iconPage * iconsPerPage
  const visibleIcons = matches.slice(offset, offset + iconsPerPage)
  return <div className="asset-panel">
    <div className="asset-toolbar"><div className="asset-modes" role="group" aria-label="Asset source"><button type="button" className="secondary" aria-pressed={mode === 'library'} disabled={busy} onClick={() => setMode('library')}>Icons</button><button type="button" className="secondary" aria-pressed={mode === 'svg'} disabled={busy} onClick={() => setMode('svg')}><ClipboardPaste size={17} />SVG</button></div><button type="button" className="secondary" disabled={busy} onClick={() => input.current?.click()}><Upload size={17} />Import files</button><input ref={input} type="file" multiple={maxFiles > 1} hidden accept=".svg,.png,.jpg,.jpeg,image/svg+xml,image/png,image/jpeg" aria-label="Import asset files" onChange={(event) => { const files = [...event.target.files ?? []]; event.target.value = ''; if (files.length) void insert(() => fileAssets(files, size)) }} /></div>
    <div className="asset-body">
      <section className="asset-library" aria-label="Asset library">{mode === 'library' ? <>
        <div className="asset-filters"><label className="field">Search icons<input autoFocus={maxFiles === 1} aria-label="Search icons" value={query} disabled={busy} onChange={(event) => { setQuery(event.target.value); goToIconPage(0) }} /></label><label className="field">Category<select aria-label="Icon category" value={category} disabled={busy} onChange={(event) => { setCategory(event.target.value); goToIconPage(0) }}><option value="all">All icons ({icons.length})</option>{categories.map((entry) => <option key={entry.id} value={entry.id}>{entry.name} ({entry.count})</option>)}</select></label></div>
        <div ref={iconGrid} className="icon-grid">{visibleIcons.map((entry) => <button type="button" key={entry.id} data-icon-name={entry.id} className={entry.id === selected.id ? 'active' : ''} aria-label={`${entry.name} icon`} aria-pressed={entry.id === selected.id} title={entry.name} disabled={busy} onClick={() => setSelected(entry)}><entry.icon size={28} /><span>{entry.name}</span></button>)}</div>
        <nav className="icon-pagination" aria-label="Icon pages"><Tool label="Previous icon page" disabled={busy || iconPage === 0} onClick={() => goToIconPage(iconPage - 1)}><ChevronLeft size={20} /></Tool><span aria-live="polite">{matches.length ? offset + 1 : 0}-{Math.min(offset + iconsPerPage, matches.length)} / {matches.length}</span><Tool label="Next icon page" disabled={busy || offset + iconsPerPage >= matches.length} onClick={() => goToIconPage(iconPage + 1)}><ChevronRight size={20} /></Tool></nav>
        {!matches.length && <p role="status">No matching icons</p>}
      </> : <label className="field">SVG markup<textarea className="code-input" aria-label="SVG markup" rows={12} maxLength={262144} disabled={busy} value={svg} onChange={(event) => setSvg(event.target.value)} /></label>}</section>
      <fieldset className="asset-properties" disabled={busy}><legend className="visually-hidden">Asset properties</legend>{mode === 'library' && <><div className="asset-preview"><selected.icon size={96} color={color} strokeWidth={stroke} aria-label={selected.name} /></div><strong>{selected.name}</strong><label className="field">Icon color<input type="color" aria-label="Icon color" value={color} onChange={(event) => setColor(event.target.value)} /></label><label className="field">Stroke width<input aria-label="Icon stroke width" type="number" min={1} max={4} step={0.5} value={stroke} onChange={(event) => setStroke(event.currentTarget.valueAsNumber)} /></label></>}{showSize && <label className="field">Size (px)<input aria-label="Asset size" type="number" min={8} max={640} required value={Number.isFinite(size) ? size : ''} onChange={(event) => setSize(event.currentTarget.valueAsNumber)} /></label>}</fieldset>
    </div>
    {error && <p className="error asset-error" role="alert">{error}</p>}
    <footer className="asset-actions"><span role="status">{busy ? 'Importing asset' : mode === 'library' ? `${matches.length} icons` : 'SVG'}</span><button className="primary" disabled={busy || !Number.isFinite(size) || size < 8 || size > 640 || mode === 'svg' && !svg.trim() || mode === 'library' && (!Number.isFinite(stroke) || stroke < 1 || stroke > 4)} onClick={() => void insert(async () => [svgAsset(mode === 'library' ? renderToStaticMarkup(<selected.icon size={24} color={color} strokeWidth={stroke} />) : svg, size, mode === 'library' ? `${selected.name} (Lucide)` : 'Imported SVG icon')])}><Check size={17} />{mode === 'library' ? 'Insert icon' : 'Insert SVG'}</button></footer>
  </div>
}