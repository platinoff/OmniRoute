# omniroute — FUNCTION_MANAGEMENT (симбіоз)

**Companion:** `S:/rust/GSV/docs/gsv/GSV_FUNCTION_MANAGEMENT.md` · [`GSV_ECOSYSTEM_SYMBIOSIS.md`](../../GSV/docs/gsv/GSV_ECOSYSTEM_SYMBIOSIS.md) · [`GSV_AGI_PATH.md`](../../GSV/docs/gsv/GSV_AGI_PATH.md)  
**Шар:** plugin (Node/TS external gateway; Rust crates additive CNV) · **Repo:** `https://github.com/platinoff/OmniRoute` · **Branch:** `release/v3.8.50`

## 1. Оголошені функції

| Функція | Модулі/шляхи | Статус | Примітка |
|---------|--------------|--------|----------|
| Unified LLM router / OpenAI-compat API | Node/TS runtime (source of truth) | active | 290+ providers; external-provider gateway lane |
| MCP/A2A/PWA surfaces | runtime/UI modules | active | Per product docs; keep additive |
| Rust CNV crates (opt-in waves) | `crates/` (`compression-core`, napi/wasm adapters, etc.) | active (bounded opt-in) | CNV-1 W1–W22 done; W23+ additive/opt-in, no default runtime wiring unless approved |
| Config/env/security policies | product config/docs | active | No secrets in git; owner-gated external keys |

## 2. Звірка / gates

- Node/TS gates per project workflow (CI/runtime source of truth).
- Rust crates: `cargo fmt --check`, `cargo clippy -D warnings`, targeted tests, wasm32 builds where applicable (per CNV wave).
- CNV policy: additive, bounded, opt-in; do not break existing npm/Node/Electron/CI flows.

## 3. Дублі / межі (посилання на master §3)

- **D2** LLM routing: **intentional parallel lane** — GSV OmniRouter = mesh/free/local lanes; omniroute = external-provider gateway. Neither absorbs the other.
- GSV owns hub MCP/tickets/keep-live; omniroute owns external gateway runtime.

## 4. Owner-рішення / наступні кроки

- Continue CNV-1 W23+ only as bounded opt-in waves with green gates.
- Keep external API keys/secrets out of repo; owner-gated runtime wiring.
- Update skeleton when runtime ownership/wave boundaries change.
