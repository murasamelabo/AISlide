# AISlide Development

- The PPTX mapping is independently implemented from public OOXML/OPC specifications. Do not copy an existing PPTX engine or replace the core with one without user approval.
- Put document behavior in `crates/aislide-core`. Studio, CLI and MCP must share it.
- Use `node tools/cargo.mjs test --workspace` and `npm run build` for the first-slice gates.
- Use the official MCP SDK; protocol reference: https://modelcontextprotocol.io/specification/ and SDK reference: https://github.com/modelcontextprotocol/typescript-sdk/tree/v1.x/docs.
- Never execute imported content or fetch external relationships. Do not overwrite source presentations.
- Do not present synthetic data as factual data, deterministic templates as AI generation, or structural validation as Office visual parity.
- Source and documentation files use UTF-8 with BOM. JSON config files must be UTF-8 without BOM because Vite/PostCSS parses them with JSON.parse.
- Do not commit, publish or choose the project license without explicit authorization.