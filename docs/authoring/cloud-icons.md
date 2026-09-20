# Local Cloud Icon Packs

## Rights And Scope

AISlide uses a uniform **local opt-in download** policy for Microsoft Azure/Entra, Amazon Web Services and Google Cloud artwork. The repository contains sanitized metadata, hashes, source URLs, vendor notices and installer source only. Never commit or bundle vendor SVG, PNG, ZIP, PDF, or source artwork with the application or its installer. No project license is selected by this feature.

`--accept-vendor-terms` acknowledges the vendor conditions for the intended use. It does **not** grant copyright, trademark, redistribution, sublicensing or endorsement rights, nor substitute for any required vendor approval. AWS/GCP bundled-library redistribution rights have not been established. The general Google product-icon guidance may require additional permission. Local download alone does not resolve that requirement.

- Microsoft: [Azure icon conditions](https://learn.microsoft.com/en-us/azure/architecture/icons/) and [Entra icon conditions](https://learn.microsoft.com/en-us/entra/architecture/architecture-icons). Permitted purposes are architecture diagrams, training and documentation. Retain full service labels; preserve colors, proportions and design. Do not crop, flip, rotate, recolor, use as AISlide branding, imply endorsement or use Microsoft product icons in marketing communications. The source archives retain their vendor terms documents; those documents are not copied to public source.
- AWS: [architecture icon conditions](https://aws.amazon.com/architecture/icons/) and [trademark conditions](https://aws.amazon.com/trademark-guidelines/). Use for permitted architecture diagrams/presentations with service identification and unchanged appearance; no implied affiliation or unrestricted library redistribution.
- Google: [Cloud icon library](https://cloud.google.com/icons), [product-icon guidance](https://partnermarketinghub.withgoogle.com/brands/google/branding-guidelines/how-to-show-googles-brand/) and [trademark rules](https://partnermarketinghub.withgoogle.com/brands/google/trademarks-and-terms/trademark-guidelines-for-proper-usage/). Respect permitted diagram/documentation purposes and any additional approval requirements; no altered branding or implied endorsement.

The ZIP dependency `fflate` 0.8.2 is SPDX `MIT`. That license applies to the ZIP library, **not** the vendor artwork. Sharp is separately governed by its own license and dependencies. Vendor artwork is not represented as MIT, Apache or public domain. Studio, SDK and MCP now share the local catalog and prepared-image APIs; Lucide's existing catalog remains available. This integration does not itself install or update the desktop application.

## Setup

Prerequisites: Node 24 and the repository's installed development dependencies, including `fflate` 0.8.2 and Sharp **0.35.4**. No Azure SDK, cloud subscription, account, authentication or Azure resource operation is involved.

```text
node tools/cloud-icons-setup.mjs --help
node tools/cloud-icons-setup.mjs --accept-vendor-terms
```

Without the acceptance flag, setup prints the vendor links/conditions, exits 1 and performs no download or pack writes. `--help` exits 0 with no pack access. Acceptance applies only to this explicit run; it does not persist global environment settings or alter the user application.

To reuse previously obtained official archives, use an **absolute local** cache directory:

```text
node tools/cloud-icons-setup.mjs --accept-vendor-terms --cache-dir=C:/approved-local-cache
```

The cache contains only these exact filenames: `azure-v24.zip`, `entra-202310.zip`, `aws-07312026.zip`, `gcp-core.zip`, `gcp-category.zip`. Existing cached files must match the exact compiled byte counts and SHA-256 pins. A corrupt file fails closed; it is not overwritten or silently replaced from the network. Missing files may be fetched from the pinned official HTTPS URLs. Downloaded archives are processed in memory, not persisted. Never place vendor artwork in public source; an ignored local cache or the user data directory is appropriate.

Exactly five archives total **23,842,521 bytes**. The installer does not fetch the GCP guide PDF, old icon packs, PPTX toolkits, third-party artwork or URLs supplied by a presentation. Requests use exact pinned URLs, reject redirects and unexpected status/size, and have a finite timeout. Archive hash verification precedes ZIP decoding; selected entry hashes precede image decoding. No archive paths are extracted to disk.

## Studio And SDK/MCP Workflow

1. Review the vendor conditions, then run the explicit setup above. The qualified local pack has 1,471 deduplicated PNG files totaling 7,664,102 bytes; 1,498 catalog identities may share artwork. Missing consent/assets fail visibly, without runtime downloads or fallback art.
2. In **Insert icons**, select **Lucide**, **Microsoft Azure / Entra**, **Amazon Web Services** or **Google Cloud**, then filter by category/search and page through 60 entries at a time. All 3,316 entries are available across providers. Cloud selections preserve colors, aspect ratio and prepared PNG bytes; color/stroke controls are hidden.
3. In **Architecture diagram**, use **Add service icon** for a 160x140 icon-first node with a 16px full-service-name label. A name over the 160-character label limit rejects, never truncates. Existing nodes offer Card/Icon presentation; Icon requires an image, and removal returns to Card. Boundaries support parent selection and choose/change/remove icon, up to 16 boundaries/four levels. Apply commits the graph once; local Undo/Redo remains 30 entries/4 MiB per direction.
4. SDK: call `client.architectureIcons(options?)`, check `configured`, then `client.architectureIconAssets(ids,options?)`. MCP: `architecture_icons` then `architecture_icon_assets` with `{ids}`. Request 1-60 distinct IDs (at most 256 bytes each); pass only `{base64,mime_type,alt}` to strict graph inputs, retaining the full service name. See the [direct icon/nested-boundary example](../../packages/client/README.md#local-cloud-icon-catalog).

`configured` verifies only acknowledgment for the exact catalog, not every file. Asset reads recheck consent, selected hashes, PNG magic/dimensions and bounds before returning the whole batch: 1 MiB per PNG, at most 512px, 4 MiB raw bytes per batch. Do not thumbnail these assets through the generic 256px `createGraphIcon` helper. Saved PPTX embeds picture bytes and editable labels/shapes plus optional graph metadata, so offline reopening does not require the pack. No runtime network or external relationships are fetched. Logical nesting is not nested PowerPoint groups; Office shape-only movement leaves separate labels/pictures behind. See [contract and routing limits](../api.md#architecture-graphs) and [current bounded qualification](../testing/graphs-and-dads.md#2026-09-20-cloud-and-workspace-qualification).

## Local Contract

Default Windows version directory:

```text
%LOCALAPPDATA%/AISlide/icon-packs/2026-09-20
```

Each image is stored at `<version-directory>/<provider>/<png_sha256>.png`. The ready acknowledgment is `<version-directory>/consent.json`:

```json
{
  "version": 1,
  "catalog_sha256": "SHA-256 of the exact public architecture-icons.json bytes",
  "accepted_vendor_terms": true
}
```

The catalog is [architecture-icons.json](../../crates/aislide-core/src/architecture-icons.json), UTF-8 **without BOM**. Its exact bytes, including whitespace, are covered by the consent hash. It has top-level `version: 1`, `release: "2026-09-20"`, `providers` and `icons`. Providers contain `id`, `name`, `terms_url`, `notice`, and `archives: [{id,url,sha256,bytes}]`. Every icon contains:

```text
id, provider, name, kind, categories, aliases,
source: {archive_id, entry, sha256},
png_sha256, png_bytes, width, height
```

This release is 1,112,527 bytes, SHA-256 `bd9db6956f49a686dd1fc6f66d0f60394e7144929ed72b91053efeee95b0a8a7`; its `.gitattributes` `-text` rule prevents CRLF conversion so consent remains portable, and any pin update requires explicit review of the exact catalog bytes.

`id` is the canonical research identity, not a filesystem path. `source.sha256` hashes the exact selected official entry; `png_sha256` hashes the prepared PNG. There are no machine-local paths, image bytes or base64 payloads in the catalog. Consumers must resolve only compiled provider/hash paths, verify consent version/catalog hash/acceptance, and verify image bytes/hash/limits. Do not accept request-supplied directories, source URLs or catalog replacements. No runtime download is required after setup.

The operator environment variable `AISLIDE_ICON_PACK_ROOT` can select an absolute local **version directory** for isolated tests/deployments. It is not a request parameter; do not persist it globally. Relative paths, URLs, UNC/device paths, traversal, symlinks and junctions are rejected. On non-Windows systems, the default base is absolute `XDG_DATA_HOME`, otherwise `HOME/.local/share`, followed by `AISlide/icon-packs/2026-09-20`.

Setup also rejects Windows remote/unknown drive types before filesystem access, using a bounded PowerShell `DriveInfo` check from trusted host `SystemRoot` configuration; neither drive choice nor shell configuration comes from a presentation request.

On Windows, runtime access also requires a known local drive type before any filesystem metadata probe or open, including consent reads and roots derived from `LOCALAPPDATA`. After lexical validation, the small `aislide-platform` crate's safe `is_local_drive` API passes only a validated ASCII drive letter in a stack UTF-16 root to one `GetDriveTypeW` call; core retains `#![forbid(unsafe_code)]`. Removable (2), fixed (3), CD-ROM (5) and RAM disk (6) pass; remote (4), unknown (0), missing-root (1) and all other values fail closed. Runtime classification uses no shell, `SystemRoot`, or subprocess and does not cache successful classifications across checks. Rejected roots leave compiled catalog metadata available with `configured: false`; asset requests still validate IDs before resolving the host root. Existing ancestor/reparse, open-file metadata, consent and PNG hash checks remain. These checks do not prevent hostile concurrent drive remapping or ancestor replacement between classification and file access.

Existing equal bytes are reused. Different existing PNGs or acknowledgments cause a clear failure and are never silently overwritten or deleted. Publication uses private staging plus exclusive hard links on the same volume, then publishes consent last after all images verify. Failed new installations can leave verified hash-named files without consent; rerunning can finish them. Only the current run's private staging is cleaned. The storage root should be owned by the local user and not concurrently manipulated by hostile processes; portable Node filesystem checks are not an adversarial directory-handle sandbox.

## Preparation And Coverage

All **1,498** source identities are retained: Azure **638** plus Entra **7** under provider `azure`, AWS **808**, and GCP **45**. Categories and resource/group glyphs are included; this is not a claim of 1,498 distinct products. Original category memberships survive. Coverage mappings provide full service names and searchable aliases; other labels remain source-derived, with provider names. GCP Networking intentionally supplies VPC and Cloud Load Balancing aliases; Security/Identity supplies IAM. The official Vertex AI source remains Vertex AI, not Gemini.

Sources are Azure V24 (July 2026), Entra October 2023, AWS `07312026`, and the current GCP new-style library (19 core + 26 category identities); legacy GCP artwork is explicitly not used. The existing Lucide 1,818 entries/42 categories are retained separately, not counted as cloud products.

Of the **853 AWS/GCP PNG sources**, **852** are preserved byte-for-byte, including 512px originals. The exception is GCP Looker: its official `Looker-512-color.png` is actually **1024x1024**, so it is uniformly resized to **256x256**, with the original source hash retained separately. PNG sources over 512px use this aspect-preserving 256px fit; none are enlarged. The **645 Azure/Entra SVGs** use the pinned, offline Sharp path, uniformly fitted inside a 256px square with original aspect ratio and full source bounds. There is no cropping, recoloring, rotation or style stripping. Local gradient references are supported; active content, external references and entity constructs are rejected. This trusted-pack path never relaxes ordinary imported-SVG guards. Every prepared PNG must decode, have nonzero alpha coverage, fit within 512px/1MiB and match its catalog pin. This does not claim visual parity with Office or certify the semantics of every filename.

Sharp 0.35.4 and its native rendering/encoding stack produced the derived hashes. Another platform or native-library build can differ even at the same Sharp version. Setup fails clearly on a derived-byte mismatch; it never silently regenerates release pins. Preserve official PNGs rather than passing them through a generic graph thumbnail helper that would change 512px originals. Consumer layout may scale display geometry without altering source bytes or appearance.

## Maintainer And Tests

Public metadata can be generated from the approved canonical research directory, containing `catalog.json`, `coverage.json`, `research-manifest.json` and the five pinned archives:

```text
node tools/cloud-icons-setup.mjs --accept-vendor-terms --build-catalog=C:/approved-canonical-research
node --test tools/cloud-icons-setup.test.mjs
```

The builder emits only the one public metadata JSON, validates all prepared hashes/dimensions/nonblank images before publication, and refuses to overwrite differing existing metadata. It does not install the local pack or create consent. Generate the catalog first, then install, so the acknowledgment binds the final metadata bytes. A future release change requires an explicit reviewed pin/metadata update; this immutable release is not an auto-updater.

For the complete offline integration test, set `AISLIDE_CLOUD_ICON_TEST_CACHE` in the test process environment to the absolute verified archive directory. That test installs all 1,498 identities to a unique temporary directory, checks every PNG's hash/dimensions/nonblank alpha, verifies consent and hash-only filenames, then repeats setup with network forbidden and verifies unchanged bytes/timestamps. It cleans only its own temporary directory. Without this explicit cache, only that full-archive integration case is skipped; synthetic safety tests still run without external network access.

The installer module exports `catalogPath`, `defaultPackDirectory({env,platform,homeDirectory})` and `installIconPacks({acceptVendorTerms,packDirectory?,cacheDirectory?,fetch?})`; options do not mutate global environment state. The result contains `directory`, `catalog_sha256`, `icons`, `files`, `written` and `ready`. `buildCatalog`, `verifyCatalog`, `downloadArchive`, `readArchiveEntries`, `prepareIconPng` and `inspectPng` are tooling/test helpers, not application import or network APIs.

## Runnable Synthetic Examples

With the consented local pack already installed, Node 24, installed repository dependencies, and an existing CLI exposing the shared cloud catalog and graph APIs, run:

```text
node tools/graphs-demo.mjs --cloud-icons
```

The demo creates a new timestamped directory under `.artifacts`; an optional new or empty output directory can be passed explicitly. It refuses nonempty directories and existing output files. `--cloud-icons` and `--icons` are mutually exclusive. No flag still produces the original five examples; `--icons` retains all 18 Lucide PNG/JPEG node icons across those five slides.

The cloud mode authors three original, synthetic diagrams, each labeled **Synthetic example / not a deployed system**:

| Slide | Coverage | Nodes / boundaries / connections |
| --- | --- | --- |
| AWS | Cloud, two regions, two VPCs, public/private subnets, EC2, NAT Gateway, Internet Gateway and Application Load Balancer; maximum boundary depth four | 6 / 9 / 5 |
| Azure | VNet integration and private-endpoint subnets, App Service, SQL Database, Key Vault, Storage account, Monitor, Web Application Firewall and the exact Entra ID catalog identity | 9 / 4 / 4 |
| Google Cloud | VPC and subnet groupings, Cloud Run, GKE, Compute Engine, Cloud SQL, Storage and IAM | 8 / 5 / 5 |

These are illustrative service relationships and logical groupings, not deployment instructions, availability claims or exhaustive network configurations. Standalone identity, monitoring and storage/security services illustrate catalog coverage without asserting every dependency. External clients use independently rendered Lucide Users artwork. Full service names remain in adjacent labels and the catalog's exact image ALT; contextual boundary names do not replace that ALT.

Google Cloud VPC, subnet and Load Balancing labels intentionally use `gcp/category/networking`; IAM uses `gcp/category/security-identity`. These are category illustrations, not claims that a dedicated product icon exists. Entra uses `azure/entra/microsoft-entra-id`. The evidence records the category mappings and all 27 selected metadata identities.

The official MCP SDK calls `architecture_icons` and `architecture_icon_assets`, then the existing graph insertion, export, native reopening, editing and Undo tools. Missing consent fails with the catalog message; there is no installer, runtime download or artwork fallback. Vendor PNGs, including 512px GCP originals, bypass `create_graph_icon` and retain their pinned bytes. Only the three GraphIcon fields (`base64`, `mime_type`, `alt`) enter a graph. No vendor artwork is added to public source.

Outputs are one three-slide `architecture-graphs.pptx`, an edited copy, a byte-identical undone copy, three offline core-rendered PNG previews, and `evidence.json`. Assertions cover selected PNG hashes/sizes/dimensions/ALT, exported native media, node and boundary geometry, parent ancestry and depth, picture/label bounds, attached native connector references, current graph metadata after reopening, a `(24,16)` first-node move, and byte-identical Undo. The evidence includes source/reopened/edited element IDs, bounds, label font sizes, image hashes and connector routes for subsequent visual review.

The previews include a nonblank colored-pixel check, not text-pixel or Office equivalence verification. Elbow routes are deterministic and do not avoid obstacles automatically. Invisible native connection anchors can reopen as empty text elements; identity, geometry and attachment checks remain mandatory. Browser/Office rendering, manual PowerPoint interaction, desktop installation and publication are separate qualification steps and are not performed by this demo.