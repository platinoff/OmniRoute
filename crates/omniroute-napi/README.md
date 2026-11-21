# omniroute-napi (CNV-1 Wave 5+)

Additive N-API bindings for OmniRoute Rust crates (`omniroute-gcf`, `omniroute-compression`, `omniroute-sse`).  
**Opt-in:** not part of root npm workspaces/build/test. Does not change existing omniroute runtime flows.

## Exposed API

- `gcfEncodeJson(jsonInput: string, noFlatten?: boolean): string` — JSON value string → GCF generic text
- `gcfDecodeToJson(gcfText: string): string` — GCF generic text → JSON value string
- `validateRulePackJson(json: string): string` — rule pack JSON → `{ valid, errors }` JSON
- `validateCompressionJson(original: string, compressed: string): string` — `{ valid, errors, warnings, fallbackApplied }` JSON
- `isCompressionExcluded(provider: string | null, model: string | null, exclusionsJson: string): boolean`
- `sseStripAnsiCodes(s: string): string`
- `sseParseDataPayload(data: string, eventType?: string | null): string | null` — parsed value as JSON string
- `sseParseDataLines(linesJson: string, eventType?: string | null): string | null` — `linesJson` is JSON array of strings
- `sseExtractDataLine(line: string): string | null`
- `sseParseLine(line: string): string | null`
- `sseFormatSse(dataJson: string | null | undefined, sourceFormat?: string | null): string` — `dataJson` is JSON string value (or null/undefined)
- `sseIsOpenaiChoicesPayload(parsedJson: string): boolean`
- `sseHasOpenaiFinishReason(parsedJson: string): boolean`
- `sseIsKnownNonClaudeStreamPayload(parsedJson: string, eventType?: string | null): boolean`
- `sseUnwrapGeminiChunk(parsedJson: string): string` — returns JSON string of unwrapped/original value
- `sseNormalizeLinesBatch(linesJson: string): string` — `linesJson`/result are JSON arrays of strings (single-batch, fresh normalizer)

## Build (local, opt-in)

```bash
# from crates/omniroute-napi
npm install
npm run build
```

Requires Rust toolchain + Node/npm + platform build tools. Uses `@napi-rs/cli` v2.
Generated `index.js`/`index.d.ts` and platform `.node` artifacts are build outputs (review before committing any generated files).

## Rust-only check

```bash
cargo check -p omniroute-napi
cargo clippy -p omniroute-napi --all-targets
```