# omniroute-napi (CNV-1 Wave 5)

Additive N-API bindings for OmniRoute Rust crates (`omniroute-gcf`, `omniroute-compression`).  
**Opt-in:** not part of root npm workspaces/build/test. Does not change existing omniroute runtime flows.

## Exposed API (foundation)

- `gcfEncodeJson(jsonInput: string, noFlatten?: boolean): string` — JSON value string → GCF generic text
- `gcfDecodeToJson(gcfText: string): string` — GCF generic text → JSON value string
- `validateRulePackJson(json: string): string` — rule pack JSON → `{ valid, errors }` JSON
- `validateCompressionJson(original: string, compressed: string): string` — `{ valid, errors, warnings, fallbackApplied }` JSON
- `isCompressionExcluded(provider: string | null, model: string | null, exclusionsJson: string): boolean`

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