# Single-file PPTX Operation

Studio now uses **Open PPTX** and **Save PPTX**. Select one `.pptx`; a matching external JSON file is no longer required. Saved presentations retain native editable text, shapes, tables, charts, pictures, masters, layouts and notes. Optional source/binding information uses a standard internal Custom XML part and never contains a cached deck snapshot.

## Safety And Boundaries

- Actual presentation XML is the source of content. Edits made outside AISlide are read from that XML, including supported local placeholder formatting.
- Unchanged native files are returned byte-for-byte. Supported modifications patch original XML; non-target parts and opaque content are retained, not executed. Preservation is not sanitization and is not a claim that every object can be rendered or edited.
- Native text edits preserve simple paragraph formatting. Whole-frame style changes require a representable style; mixed/extended formatting is rejected when it would be lost. Custom chart settings are preserved; data replacement is permitted only when the original chart is fully represented by the supported native writer.
- Supported master/layout contents, theme colors/fonts, table cells, shape geometry, new shapes, pictures, charts and hyperlinks can be edited. Supported reopened decks also allow slide insertion, duplication, removal, ordering and renaming; sections/custom shows and references to deleted slides are guarded. Master/layout counts remain fixed. Unsupported actions fail before the current document changes.
- Source metadata is unkeyed integrity information, not authentication or proof of factual accuracy. Changed source-bound values become stale. Source content remains private document content even though no separate JSON is visible.
- Only unencrypted macro-free Transitional Open XML PPTX is handled by the new reader. OLE containers, legacy `.ppt`, macro presentations and the separate Strict namespace profile are not silently converted. No protection, tenant policy or sensitivity label is removed.
- New file publication refuses existing destinations and checks the persisted bytes immediately. An external process or organizational protection policy can still change a file later; a successful save cannot guarantee that a future file remains unencrypted.
- Normal source/scene limits still apply: 1280x720 normalized editor coordinates, 1-32 slides, bounded objects/resources and request sizes. Native coordinate writes are scaled to the original page's EMU dimensions.

## Verification

The shared Rust single-file tests exercise standalone reopening, native theme/layout/preset restoration, actual XML edits taking precedence, no-op byte identity, source metadata round trips, text/style/geometry/table/theme/master updates, image/chart/link insertion, original coordinate units and rejection of lossy edits. OLE inputs produce a protection/legacy-format message rather than a checkpoint mismatch.

SDK, MCP and browser tests exercise one-file save/reopen, source retention, direct editing, undo, no overwrite and keeping the active document after a failed open. The native desktop scenario tests the same workflow in an owned WebView/profile. Legacy checkpoint tests remain to protect the compatibility API.

The public source includes a synthetic single-PPTX qualification generator:

```sh
npm run core:build
node tools/workspace-demo.mjs
```

An earlier, locally retained Agent 365 sample passed official Open XML SDK validation, all 12 PowerPoint slide renders, six edit/undo checks and exact reproduction after reopening from the PPTX alone at generation time. Task-specific generators and deliverables are not included in public source. These are bounded compatibility checks, not universal Office visual parity. The original protected/mismatched presentations were not overwritten.

**Later file-state check, 2026-09-14:** the previously validated Agent 365 single-file artifact had subsequently become an OLE container (`d0cf11e0a1b11ae1`, 428032 bytes, SHA-256 `450aebbec3501b8bc36b87429408a9e5991460cd7044a748d451b801d7fd0ca6`). The actor/cause is not established. It is not currently claimed to be an ordinary Open XML ZIP; no overwrite, decryption, labeling change or policy bypass was performed. The separately generated [parts qualification](parts-library.md) and [workspace qualification](workspace-ux.md) cover current standard-PPTX examples.

Previously generated `.aislide.json` files are not automatically trusted or used to repair a changed PPTX. The legacy API still requires exact matching bytes; use the new standard-PPTX opening flow instead when the actual file is readable. Protected files require an authorized workflow outside AISlide, not a hash-check bypass.