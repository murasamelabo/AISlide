import { useRef, useState } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { Database, Server, Cloud, Globe, Router, Monitor, Smartphone, Folder, FileText, Mail, User, Users, ShieldCheck, LockKeyhole, KeyRound, Check, CircleAlert, Search, Settings, Workflow, Network, GitBranch, ArrowRight, ArrowLeftRight, Calendar, Clock, ChartNoAxesCombined, Building2, Briefcase, Layers, Link, MessageSquare, Upload, Download, HardDrive, Bell, ClipboardPaste } from 'lucide-react'
import type { AssetInput } from './types'
import { fileAssets, svgAsset } from './api'

const icons = [
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
]

export function AssetPanel({ onInsert, onBusy }: { onInsert: (assets: AssetInput[]) => Promise<void>; onBusy: (busy: boolean) => void }) {
  const [query, setQuery] = useState('')
  const [selected, setSelected] = useState(icons[0])
  const [color, setColor] = useState('#0017c1')
  const [size, setSize] = useState(96)
  const [stroke, setStroke] = useState(2)
  const [mode, setMode] = useState<'library' | 'svg'>('library')
  const [svg, setSvg] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const input = useRef<HTMLInputElement>(null)
  const gate = useRef(false)
  async function insert(action: () => Promise<AssetInput[]>) {
    if (gate.current) return
    gate.current = true; setBusy(true); onBusy(true); setError('')
    try { await onInsert(await action()) }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)) }
    finally { gate.current = false; setBusy(false); onBusy(false) }
  }
  const matches = icons.filter((entry) => `${entry.name} ${entry.tags}`.toLowerCase().includes(query.trim().toLowerCase()))
  return <div className="asset-panel">
    <div className="asset-toolbar"><div className="asset-modes" role="group" aria-label="Asset source"><button className="secondary" aria-pressed={mode === 'library'} disabled={busy} onClick={() => setMode('library')}>Icons</button><button className="secondary" aria-pressed={mode === 'svg'} disabled={busy} onClick={() => setMode('svg')}><ClipboardPaste size={17} />SVG</button></div><button className="secondary" disabled={busy} onClick={() => input.current?.click()}><Upload size={17} />Import files</button><input ref={input} type="file" multiple hidden accept=".svg,.png,.jpg,.jpeg,image/svg+xml,image/png,image/jpeg" aria-label="Import asset files" onChange={(event) => { const files = [...event.target.files ?? []]; event.target.value = ''; if (files.length) void insert(() => fileAssets(files, size)) }} /></div>
    <div className="asset-body"><section className="asset-library" aria-label="Asset library">{mode === 'library' ? <><label className="field">Search icons<input aria-label="Search icons" value={query} disabled={busy} onChange={(event) => setQuery(event.target.value)} /></label><div className="icon-grid">{matches.map((entry) => <button type="button" key={entry.name} className={entry.name === selected.name ? 'active' : ''} aria-label={`${entry.name} icon`} aria-pressed={entry.name === selected.name} title={entry.name} disabled={busy} onClick={() => setSelected(entry)}><entry.icon size={28} /><span>{entry.name}</span></button>)}</div>{!matches.length && <p role="status">No matching icons</p>}</> : <label className="field">SVG markup<textarea className="code-input" aria-label="SVG markup" rows={12} maxLength={262144} disabled={busy} value={svg} onChange={(event) => setSvg(event.target.value)} /></label>}</section><fieldset className="asset-properties" disabled={busy}><legend className="visually-hidden">Asset properties</legend>{mode === 'library' && <><div className="asset-preview"><selected.icon size={96} color={color} strokeWidth={stroke} aria-label={selected.name} /></div><strong>{selected.name}</strong><label className="field">Icon color<input type="color" aria-label="Icon color" value={color} onChange={(event) => setColor(event.target.value)} /></label><label className="field">Stroke width<input aria-label="Icon stroke width" type="number" min={1} max={4} step={0.5} value={stroke} onChange={(event) => setStroke(event.currentTarget.valueAsNumber)} /></label></>}<label className="field">Size (px)<input aria-label="Asset size" type="number" min={8} max={640} required value={Number.isFinite(size) ? size : ''} onChange={(event) => setSize(event.currentTarget.valueAsNumber)} /></label></fieldset></div>
    {error && <p className="error asset-error" role="alert">{error}</p>}
    <footer className="asset-actions"><span role="status">{busy ? 'Importing asset' : mode === 'library' ? `${matches.length} icons` : 'SVG'}</span><button className="primary" disabled={busy || !Number.isFinite(size) || size < 8 || size > 640 || mode === 'svg' && !svg.trim() || mode === 'library' && (!Number.isFinite(stroke) || stroke < 1 || stroke > 4)} onClick={() => void insert(async () => [svgAsset(mode === 'library' ? renderToStaticMarkup(<selected.icon size={24} color={color} strokeWidth={stroke} />) : svg, size, mode === 'library' ? `${selected.name} (Lucide)` : 'Imported SVG icon')])}><Check size={17} />{mode === 'library' ? 'Insert icon' : 'Insert SVG'}</button></footer>
  </div>
}