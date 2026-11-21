# omniroute-wasm (CNV-1, opt-in)

Additive WebAssembly bindings for selected OmniRoute Rust crates (`omniroute-gcf`, `omniroute-sse`).  
Not wired into default omniroute runtime/build/test flows.

## Exposed API (JSON/string boundary)

- `gcfEncodeJson(jsonInput, noFlatten?) -> string`
- `gcfDecodeToJson(gcfText) -> string`
- `sseStripAnsiCodes(s) -> string`
- `sseParseDataPayload(data, eventType?) -> string | null`
- `sseParseDataLines(linesJson, eventType?) -> string | null`
- `sseExtractDataLine(line) -> string | null`
- `sseParseLine(line) -> string | null`
- `sseFormatSse(dataJson?, sourceFormat?) -> string`
- `sseIsOpenaiChoicesPayload(parsedJson) -> boolean`
- `sseHasOpenaiFinishReason(parsedJson) -> boolean`
- `sseIsKnownNonClaudeStreamPayload(parsedJson, eventType?) -> boolean`
- `sseUnwrapGeminiChunk(parsedJson) -> string`
- `sseNormalizeLinesBatch(linesJson) -> string`
- `validateCompressionJson(original, compressed) -> string` (`{valid,errors,warnings,fallbackApplied}`)
- `validateRulePackJson(json) -> string` (`{valid,errors}`)
- `isCompressionExcluded(provider?, model?, exclusionsJson) -> boolean`
- `sseAppendBoundedText(current, next) -> string`
- `sseHasActiveDeltaValue(valueJson) -> boolean`
- `sseFixInvalidId(parsedJson, fallbackId?) -> string` (`{"value","changed"}`)
- `sseBuildSyntheticChatChunk(id, model, created, deltaJson, finishReasonJson?) -> string` (`created` as JS number, unix seconds)
- `SseDataLineNormalizerState` — `new()`, `hasPending()`, `normalize(lines: string[])`
- `SseEventPrefixBufferState` — `new()`, `clear()`, `eventType()`, `flush()`, `prefixData(output,line)`, `remember(line)`

## Build (opt-in)

Requires `wasm32-unknown-unknown` target and matching `wasm-bindgen` CLI.

```bash
cd /s/rust/omniroute
cargo build -p omniroute-wasm --target wasm32-unknown-unknown --release
wasm-bindgen --target nodejs \
  --out-dir crates/omniroute-wasm/pkg \
  target/wasm32-unknown-unknown/release/omniroute_wasm.wasm
```

Generated `pkg/` is gitignored.

**Node ESM projects note:** if the repo root has `"type": "module"`, Node may treat generated `pkg/*.js` as ESM. For `--target nodejs` (CJS output), add `crates/omniroute-wasm/pkg/package.json` with `{"type":"commonjs"}` (or use an appropriate wasm-bindgen target) when consuming from ESM contexts.
