/**
 * Opt-in native Rust bridge (CNV-1 W5/W6).
 *
 * Loads the napi-rs addon from `crates/omniroute-napi` when available and
 * exposes wrappers over ported Rust primitives (GCF, validation, exclusions,
 * rule-pack validation). This module is NOT imported by default compression
 * paths and is NOT exported from `compression/index.ts`.
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

/** Low-level access for advanced/experimental use. */
export function getNativeBinding(): NativeBinding {
  return binding();
}