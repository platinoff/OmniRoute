/**
 * Opt-in native Rust bridge (CNV-1 W5+).
 *
 * Loads the napi-rs addon from `crates/omniroute-napi` when available and
 * exposes wrappers over ported Rust primitives (GCF, compression validation,
 * exclusions, rule-pack validation, SSE/protocol helpers). This module is NOT
 * imported by default runtime paths and is NOT exported from `compression/index.ts`.
 *
 * Enable/use explicitly. Default runtime behavior remains TS implementations.
 */

import { createRequire } from "node:module";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

type NativeBinding = {
  gcfEncodeJson(jsonInput: string, noFlatten?: boolean): string;
  gcfDecodeToJson(gcfText: string): string;
  validateRulePackJson(json: string): string;
  validateCompressionJson(original: string, compressed: string): string;
  isCompressionExcluded(
    provider: string | null | undefined,
    model: string | null | undefined,
    exclusionsJson: string
  ): boolean;
  // SSE/protocol primitives (W8+ opt-in)
  sseStripAnsiCodes?(s: string): string;
  sseParseDataPayload?(data: string, eventType?: string | null): string | null;
  sseParseDataLines?(linesJson: string, eventType?: string | null): string | null;
  sseExtractDataLine?(line: string): string | null;
  sseParseLine?(line: string): string | null;
  sseFormatSse?(dataJson: string | null | undefined, sourceFormat?: string | null): string;
  sseIsOpenaiChoicesPayload?(parsedJson: string): boolean;
  sseHasOpenaiFinishReason?(parsedJson: string): boolean;
  sseIsKnownNonClaudeStreamPayload?(parsedJson: string, eventType?: string | null): boolean;
  sseUnwrapGeminiChunk?(parsedJson: string): string;
  sseNormalizeLinesBatch?(linesJson: string): string;
  sseAppendBoundedText?(current: string, next: string): string;
  sseHasActiveDeltaValue?(valueJson: string): boolean;
  sseFixInvalidId?(parsedJson: string, fallbackId?: string | null): string;
  sseBuildSyntheticChatChunk?(
    id: string,
    model: string,
    created: number,
    deltaJson: string,
    finishReasonJson?: string | null
  ): string;
  SseDataLineNormalizerState?: new () => {
    hasPending(): boolean;
    normalize(lines: string[]): string[];
  };
  SseEventPrefixBufferState?: new () => {
    clear(): void;
    eventType(): string;
    flush(): string;
    prefixData(output: string, line: string): string;
    remember(line: string): void;
  };
  // Compression core helpers (W17+, opt-in)
  compressionIsTextBlock?(valueJson: string): boolean;
  compressionExtractTextContent?(contentJson: string | null | undefined): string;
  compressionReplaceTextContent?(msgJson: string, newText: string): string;
  compressionDetectLanguage?(text: string): string;
  compressionListSupportedLanguages?(): string[];
  compressionScoreToken?(token: string): number;
  compressionPruneByScore?(text: string, keepRate: number, minScore: number): string;
};

let cachedBinding: NativeBinding | null | undefined;
let cachedError: Error | null = null;

function thisDir(): string {
  return path.dirname(fileURLToPath(import.meta.url));
}

function candidatePaths(): string[] {
  const envPath = process.env.OMNIROUTE_RUST_NAPI_PATH;
  const base = path.resolve(thisDir(), "../../../crates/omniroute-napi");
  const paths: string[] = [];
  if (envPath) paths.push(path.resolve(envPath));
  paths.push(
    path.join(base, "index.js"),
    path.join(base, "omniroute_napi.node"),
    path.join(base, "omniroute-napi.node"),
    path.join(base, "omniroute_napi.win32-x64-msvc.node"),
    path.join(base, "omniroute_napi.win32-x64-gnu.node"),
    path.join(base, "omniroute_napi.linux-x64-gnu.node"),
    path.join(base, "omniroute_napi.darwin-x64.node"),
    path.join(base, "omniroute_napi.darwin-arm64.node")
  );
  // Also allow loading from napi-generated platform package locations if present.
  paths.push(path.join(base, "npm", "win32-x64-msvc", "omniroute_napi.win32-x64-msvc.node"));
  paths.push(path.join(base, "npm", "linux-x64-gnu", "omniroute_napi.linux-x64-gnu.node"));
  return paths;
}

function tryLoad(): NativeBinding | null {
  if (cachedBinding !== undefined) return cachedBinding;
  const require = createRequire(import.meta.url);
  const errors: string[] = [];
  for (const p of candidatePaths()) {
    try {
      if (!existsSync(p)) continue;
      // eslint-disable-next-line @typescript-eslint/no-var-requires
      const mod = require(p) as NativeBinding;
      if (mod && typeof mod.gcfEncodeJson === "function") {
        cachedBinding = mod;
        cachedError = null;
        return cachedBinding;
      }
      errors.push(`${p}: missing expected exports`);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      errors.push(`${p}: ${msg}`);
    }
  }
  cachedBinding = null;
  cachedError = new Error(
    `omniroute Rust N-API binding not available (opt-in). Build crates/omniroute-napi or set OMNIROUTE_RUST_NAPI_PATH. Attempts: ${errors.join(" | ") || "no candidate files found"}`
  );
  return null;
}

export function isRustBridgeAvailable(): boolean {
  return tryLoad() !== null;
}

export function getRustBridgeLoadError(): string | null {
  tryLoad();
  return cachedError ? cachedError.message : null;
}

function binding(): NativeBinding {
  const b = tryLoad();
  if (!b) throw cachedError ?? new Error("omniroute Rust N-API binding not available");
  return b;
}

export interface NativeValidationResult {
  valid: boolean;
  errors: string[];
  warnings: string[];
  fallbackApplied: boolean;
}

export interface NativeRulePackValidation {
  valid: boolean;
  errors: string[];
}

/** GCF generic encode (Rust). Mirrors TS `encodeGeneric(data, opts?)` shape at value level. */
export function encodeGenericNative(data: unknown, opts?: { noFlatten?: boolean }): string {
  const json = JSON.stringify(data);
  return binding().gcfEncodeJson(json, opts?.noFlatten);
}

/** GCF generic decode (Rust). Mirrors TS `decodeGeneric(input)` result shape. */
export function decodeGenericNative(input: string): unknown {
  const json = binding().gcfDecodeToJson(input);
  return JSON.parse(json) as unknown;
}

/** Compression validation (Rust). Mirrors TS `validateCompression` result shape. */
export function validateCompressionNative(original: string, compressed: string): NativeValidationResult {
  const json = binding().validateCompressionJson(original, compressed);
  return JSON.parse(json) as NativeValidationResult;
}

/** Rule pack validation (Rust). Mirrors TS `validateRulePack` structural result shape. */
export function validateRulePackNative(pack: unknown): NativeRulePackValidation {
  const json = binding().validateRulePackJson(JSON.stringify(pack));
  return JSON.parse(json) as NativeRulePackValidation;
}

/** Compression exclusion check (Rust). */
export function isCompressionExcludedNative(
  target: { provider?: string | null; model?: string | null },
  exclusions: string[] | undefined
): boolean {
  const list = Array.isArray(exclusions) ? exclusions : [];
  return binding().isCompressionExcluded(target.provider ?? null, target.model ?? null, JSON.stringify(list));
}

/** SSE: strip ANSI/VT100 escapes and stray C0 controls (preserves \t, \n, \r). */
export function stripAnsiCodesNative(s: string): string {
  const fn = binding().sseStripAnsiCodes;
  if (typeof fn !== "function") throw new Error("native sseStripAnsiCodes not available (rebuild addon)");
  return fn(s);
}

/** SSE: parse a data payload string to a value (or null). */
export function parseSseDataPayloadNative(data: string, eventType?: string | null): unknown | null {
  const fn = binding().sseParseDataPayload;
  if (typeof fn !== "function") throw new Error("native sseParseDataPayload not available (rebuild addon)");
  const json = fn(data, eventType ?? null);
  return json == null ? null : (JSON.parse(json) as unknown);
}

/** SSE: parse joined data lines (JSON array of strings) to a value (or null). */
export function parseSseDataLinesNative(lines: string[], eventType?: string | null): unknown | null {
  const fn = binding().sseParseDataLines;
  if (typeof fn !== "function") throw new Error("native sseParseDataLines not available (rebuild addon)");
  const json = fn(JSON.stringify(lines), eventType ?? null);
  return json == null ? null : (JSON.parse(json) as unknown);
}

/** SSE: extract payload text from a `data:` line, or null. */
export function extractSseDataLineNative(line: string): string | null {
  const fn = binding().sseExtractDataLine;
  if (typeof fn !== "function") throw new Error("native sseExtractDataLine not available (rebuild addon)");
  return fn(line);
}

/** SSE: parse a single SSE line to a value (or null). */
export function parseSseLineNative(line: string): unknown | null {
  const fn = binding().sseParseLine;
  if (typeof fn !== "function") throw new Error("native sseParseLine not available (rebuild addon)");
  const json = fn(line);
  return json == null ? null : (JSON.parse(json) as unknown);
}

/** SSE: format an SSE frame string. */
export function formatSseNative(data: unknown | null | undefined, sourceFormat?: string | null): string {
  const fn = binding().sseFormatSse;
  if (typeof fn !== "function") throw new Error("native sseFormatSse not available (rebuild addon)");
  const payload = data === undefined || data === null ? null : JSON.stringify(data);
  return fn(payload, sourceFormat ?? null);
}

/** SSE: true when payload carries an OpenAI `choices` array. */
export function isOpenaiChoicesPayloadNative(parsed: unknown): boolean {
  const fn = binding().sseIsOpenaiChoicesPayload;
  if (typeof fn !== "function") throw new Error("native sseIsOpenaiChoicesPayload not available (rebuild addon)");
  return fn(JSON.stringify(parsed));
}

/** SSE: true when any OpenAI choice carries a non-null `finish_reason`. */
export function hasOpenaiFinishReasonNative(parsed: unknown): boolean {
  const fn = binding().sseHasOpenaiFinishReason;
  if (typeof fn !== "function") throw new Error("native sseHasOpenaiFinishReason not available (rebuild addon)");
  return fn(JSON.stringify(parsed));
}

/** SSE: true when payload is a known non-Claude stream payload with relevant value. */
export function isKnownNonClaudeStreamPayloadNative(parsed: unknown, eventType?: string | null): boolean {
  const fn = binding().sseIsKnownNonClaudeStreamPayload;
  if (typeof fn !== "function") throw new Error("native sseIsKnownNonClaudeStreamPayload not available (rebuild addon)");
  return fn(JSON.stringify(parsed), eventType ?? null);
}

/** SSE: unwrap Gemini envelope when top-level candidates are absent. */
export function unwrapGeminiChunkNative(parsed: unknown): unknown {
  const fn = binding().sseUnwrapGeminiChunk;
  if (typeof fn !== "function") throw new Error("native sseUnwrapGeminiChunk not available (rebuild addon)");
  return JSON.parse(fn(JSON.stringify(parsed))) as unknown;
}

/** SSE: normalize lines in a single batch (fresh normalizer). */
export function normalizeSseLinesBatchNative(lines: string[]): string[] {
  const fn = binding().sseNormalizeLinesBatch;
  if (typeof fn !== "function") throw new Error("native sseNormalizeLinesBatch not available (rebuild addon)");
  return JSON.parse(fn(JSON.stringify(lines))) as string[];
}

/** SSE: append bounded streamed text (tail-capped). */
export function appendBoundedTextNative(current: string, next: string): string {
  const fn = binding().sseAppendBoundedText;
  if (typeof fn !== "function") throw new Error("native sseAppendBoundedText not available (rebuild addon)");
  return fn(current, next);
}

/** SSE: recursively check whether a delta-like value contains meaningful content. */
export function hasActiveDeltaValueNative(value: unknown): boolean {
  const fn = binding().sseHasActiveDeltaValue;
  if (typeof fn !== "function") throw new Error("native sseHasActiveDeltaValue not available (rebuild addon)");
  return fn(JSON.stringify(value));
}

/** SSE: fix invalid OpenAI-style chunk id deterministically. */
export function fixInvalidIdNative(
  parsed: unknown,
  fallbackId?: string | null
): { value: unknown; changed: boolean } {
  const fn = binding().sseFixInvalidId;
  if (typeof fn !== "function") throw new Error("native sseFixInvalidId not available (rebuild addon)");
  return JSON.parse(fn(JSON.stringify(parsed), fallbackId ?? null)) as { value: unknown; changed: boolean };
}

/** SSE: build a deterministic synthetic OpenAI-shaped chat completion chunk. */
export function buildSyntheticChatChunkNative(
  id: string,
  model: string,
  created: number,
  delta: unknown,
  finishReason?: unknown | null
): unknown {
  const fn = binding().sseBuildSyntheticChatChunk;
  if (typeof fn !== "function") throw new Error("native sseBuildSyntheticChatChunk not available (rebuild addon)");
  const finish = finishReason === undefined || finishReason === null ? null : JSON.stringify(finishReason);
  return JSON.parse(fn(id, model, created, JSON.stringify(delta), finish)) as unknown;
}

// ---------- Compression core helpers (W17+, opt-in) ----------

export function compressionIsTextBlockNative(value: unknown): boolean {
  const fn = binding().compressionIsTextBlock;
  if (typeof fn !== "function") throw new Error("native compressionIsTextBlock not available (rebuild addon)");
  return fn(JSON.stringify(value));
}

export function compressionExtractTextContentNative(content: unknown | null | undefined): string {
  const fn = binding().compressionExtractTextContent;
  if (typeof fn !== "function") throw new Error("native compressionExtractTextContent not available (rebuild addon)");
  const payload = content === undefined || content === null ? null : JSON.stringify(content);
  return fn(payload);
}

export function compressionReplaceTextContentNative(msg: unknown, newText: string): unknown {
  const fn = binding().compressionReplaceTextContent;
  if (typeof fn !== "function") throw new Error("native compressionReplaceTextContent not available (rebuild addon)");
  return JSON.parse(fn(JSON.stringify(msg), newText)) as unknown;
}

export function compressionDetectLanguageNative(text: string): string {
  const fn = binding().compressionDetectLanguage;
  if (typeof fn !== "function") throw new Error("native compressionDetectLanguage not available (rebuild addon)");
  return fn(text);
}

export function compressionListSupportedLanguagesNative(): string[] {
  const fn = binding().compressionListSupportedLanguages;
  if (typeof fn !== "function") throw new Error("native compressionListSupportedLanguages not available (rebuild addon)");
  return fn();
}

export function compressionScoreTokenNative(token: string): number {
  const fn = binding().compressionScoreToken;
  if (typeof fn !== "function") throw new Error("native compressionScoreToken not available (rebuild addon)");
  return fn(token);
}

export function compressionPruneByScoreNative(text: string, keepRate: number, minScore: number): string {
  const fn = binding().compressionPruneByScore;
  if (typeof fn !== "function") throw new Error("native compressionPruneByScore not available (rebuild addon)");
  return fn(text, keepRate, minScore);
}

/** Low-level access for advanced/experimental use. */
export function getNativeBinding(): NativeBinding {
  return binding();
}