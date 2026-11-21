#!/usr/bin/env node
/**
 * Opt-in smoke test for CNV-1 Rust N-API bridge (W5/W6).
 * Does not run in default CI/test flows.
 *
 * Usage (tsx, since adapter is TypeScript):
 *   npx --no-install tsx scripts/cnv1-smoke-rust-bridge.mjs
 *   ./node_modules/.bin/tsx scripts/cnv1-smoke-rust-bridge.mjs
 *   OMNIROUTE_RUST_NAPI_PATH=/abs/path/to/index.js ./node_modules/.bin/tsx scripts/cnv1-smoke-rust-bridge.mjs
 *
 * Requires built native addon in crates/omniroute-napi (see crates/omniroute-napi/README.md).
 * Exit 2 = bridge unavailable (expected until addon is built).
 */

import {
  decodeGenericNative,
  encodeGenericNative,
  getRustBridgeLoadError,
  isCompressionExcludedNative,
  isRustBridgeAvailable,
  validateCompressionNative,
  validateRulePackNative,
} from "../open-sse/services/compression/nativeRust.ts";

function assert(cond, msg) {
  if (!cond) throw new Error(msg);
}

function main() {
  if (!isRustBridgeAvailable()) {
    console.error("RUST_BRIDGE_UNAVAILABLE");
    console.error(getRustBridgeLoadError() || "unknown load error");
    process.exit(2);
  }

  const data = [
    { id: 1, status: "x", code: 1, meta: { note: "ERR[404]: Not Found" } },
    { id: 2, status: "y", code: 2, meta: { note: "[Speaker 1]: Hello world" } },
  ];
  const encoded = encodeGenericNative(data);
  const decoded = decodeGenericNative(encoded);
  assert(JSON.stringify(decoded) === JSON.stringify(data), "GCF round-trip mismatch");
  console.log("gcf_roundtrip_ok");

  const v = validateCompressionNative("Use `exact_token`.", "Use token.");
  assert(v.valid === false && v.fallbackApplied === true, "validation expected failure");
  console.log("validation_ok");

  const rp = validateRulePackNative({
    language: "xx",
    category: "filler",
    rules: [{ name: "x", pattern: "\\btest\\b", replacement: "" }],
  });
  assert(rp.valid === true, "rule pack expected valid");
  console.log("rule_pack_ok");

  const ex = isCompressionExcludedNative({ provider: "openai", model: "gpt-5.6" }, ["gpt-5.6"]);
  assert(ex === true, "exclusion expected true");
  console.log("exclusion_ok");

  console.log("CNV1_RUST_BRIDGE_SMOKE_OK");
}

try {
  main();
} catch (e) {
  console.error("CNV1_RUST_BRIDGE_SMOKE_FAIL", e instanceof Error ? e.message : String(e));
  process.exit(1);
}