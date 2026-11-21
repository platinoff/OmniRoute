#!/usr/bin/env node
/**
 * Opt-in smoke test for CNV-1 Rust N-API bridge (W5+).
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
  appendBoundedTextNative,
  buildSyntheticChatChunkNative,
  compressionDetectLanguageNative,
  compressionExtractTextContentNative,
  compressionIsTextBlockNative,
  compressionListSupportedLanguagesNative,
  compressionPruneByScoreNative,
  compressionReplaceTextContentNative,
  compressionScoreTokenNative,
  decodeGenericNative,
  encodeGenericNative,
  extractSseDataLineNative,
  fixInvalidIdNative,
  formatSseNative,
  getRustBridgeLoadError,
  hasActiveDeltaValueNative,
  hasOpenaiFinishReasonNative,
  isCompressionExcludedNative,
  isKnownNonClaudeStreamPayloadNative,
  isOpenaiChoicesPayloadNative,
  isRustBridgeAvailable,
  normalizeSseLinesBatchNative,
  parseSseDataPayloadNative,
  parseSseLineNative,
  stripAnsiCodesNative,
  unwrapGeminiChunkNative,
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

  const stripped = stripAnsiCodesNative("\x1b[2Kdata: hi");
  assert(stripped === "data: hi", "sse strip mismatch");
  const payload = parseSseDataPayloadNative(' {"a":1} ', "evt");
  assert(JSON.stringify(payload) === JSON.stringify({ a: 1, type: "evt" }), "sse payload mismatch");
  const line = parseSseLineNative("data: [DONE]");
  assert(JSON.stringify(line) === JSON.stringify({ done: true }), "sse line mismatch");
  const extracted = extractSseDataLineNative("  data:  x\r");
  assert(extracted === "x", "sse extract mismatch");
  const frame = formatSseNative({ done: true }, null);
  assert(frame === "data: [DONE]\n\n", "sse format mismatch");
  assert(isOpenaiChoicesPayloadNative({ choices: [] }) === true, "sse choices predicate mismatch");
  assert(
    hasOpenaiFinishReasonNative({ choices: [{ finish_reason: "stop" }] }) === true,
    "sse finish reason mismatch"
  );
  assert(
    isKnownNonClaudeStreamPayloadNative({ choices: [{ delta: { content: "hi" } }] }) === true,
    "sse non-claude payload mismatch"
  );
  const unwrapped = unwrapGeminiChunkNative({ response: { candidates: [{ content: { parts: [{ text: "x" }] } }] } });
  assert(JSON.stringify(unwrapped) === JSON.stringify({ candidates: [{ content: { parts: [{ text: "x" }] } }] }), "sse unwrap mismatch");
  const normalized = normalizeSseLinesBatchNative(["data: {\"a\":", "data: 1}", ""]);
  assert(JSON.stringify(normalized) === JSON.stringify(["data: {\"a\":1}", ""]), "sse normalize batch mismatch");
  assert(appendBoundedTextNative("ab", "cd") === "abcd", "sse append bounded mismatch");
  assert(hasActiveDeltaValueNative({ a: { b: "x" } }) === true, "sse active delta mismatch");
  const fixed = fixInvalidIdNative({ id: "chat", extend_fields: { requestId: "r1" } }, "fb");
  assert(fixed.changed === true && fixed.value?.id === "chatcmpl-r1", "sse fix id mismatch");
  const synthetic = buildSyntheticChatChunkNative("chatcmpl-x", "m", 1, { content: "hi" }, "stop");
  assert(synthetic?.id === "chatcmpl-x" && synthetic?.created === 1, "sse synthetic chunk mismatch");
  console.log("sse_ok");

  // Compression core helpers (W17+, opt-in)
  assert(compressionIsTextBlockNative({ type: "text", text: "hi" }) === true, "compression text block mismatch");
  assert(compressionIsTextBlockNative({ type: "image_url" }) === false, "compression non-text block mismatch");
  assert(
    compressionExtractTextContentNative([
      { type: "text", text: "a" },
      { type: "image_url", image_url: { url: "x" } },
      { type: "text", text: "b" },
    ]) === "a\nb",
    "compression extract content mismatch"
  );
  const replaced = compressionReplaceTextContentNative(
    { role: "user", content: [{ type: "text", text: "old" }, { type: "image_url", image_url: { url: "x" } }] },
    "new"
  );
  assert(replaced?.content?.[0]?.text === "new" && replaced?.content?.[1]?.type === "image_url", "compression replace mismatch");
  assert(compressionDetectLanguageNative("这是中文内容") === "zh", "compression detect language mismatch");
  const langs = compressionListSupportedLanguagesNative();
  assert(Array.isArray(langs) && langs.includes("en") && langs.includes("zh") && langs.includes("ja"), "compression languages mismatch");
  assert(compressionScoreTokenNative("https://x") === 1.0, "compression score token mismatch");
  const pruned = compressionPruneByScoreNative("This is a very simple test for compression", 1.0, 0.3);
  assert(pruned === "This is a very simple test for compression", "compression prune mismatch");
  console.log("compression_core_ok");

  console.log("CNV1_RUST_BRIDGE_SMOKE_OK");
}

try {
  main();
} catch (e) {
  console.error("CNV1_RUST_BRIDGE_SMOKE_FAIL", e instanceof Error ? e.message : String(e));
  process.exit(1);
}