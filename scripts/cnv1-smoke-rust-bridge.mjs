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
  compressionApplyCavemanOutputModeNative,
  compressionBackOffToWordBoundaryNative,
  compressionBuildCavemanOutputInstructionNative,
  compressionCollapseNewlineRunsNative,
  compressionCollapseWhitespaceNative,
  compressionCompressToolResultsNative,
  compressionDedupSystemPromptNative,
  compressionDetectLanguageNative,
  compressionExtractTextContentNative,
  compressionIsTextBlockNative,
  compressionListSupportedLanguagesNative,
  compressionNormalizeMessageWhitespaceNative,
  compressionPruneByScoreNative,
  compressionRemoveRedundantContentNative,
  compressionReplaceImageUrlsNative,
  compressionReplaceTextContentNative,
  compressionEstimateTokensNative,
  compressionEstimateTokenSavingsNative,
  compressionRatioNative,
  compressionSavingsNative,
  compressionHasNonEmptyReasoningContentNative,
  compressionEnsureReasoningContentOnToolCallsNative,
  compressionExtractReasoningContentNative,
  compressionDropReasoningContentNative,
  compressionScoreTokenNative,
  compressionShouldBypassCavemanOutputModeNative,
  compressionTrimTrailingHorizontalWhitespaceNative,
  compressionTruncateToolContentNative,
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

  // Compression output-mode + lite text primitives (W19+, opt-in)
  const instr = compressionBuildCavemanOutputInstructionNative(true, "full", false, "en");
  assert(instr.includes("[OmniRoute Caveman Output Mode]") && instr.includes("smart caveman"), "output mode instruction mismatch");
  assert(
    compressionShouldBypassCavemanOutputModeNative([{ role: "user", content: "please delete the database" }]) ===
      "irreversible_action",
    "output mode bypass mismatch"
  );
  const applied = compressionApplyCavemanOutputModeNative(
    { messages: [{ role: "user", content: "hi" }] },
    true,
    "lite",
    false,
    "en"
  );
  assert(applied.applied === true && applied.body?.messages?.[0]?.role === "system", "output mode apply mismatch");
  assert(compressionTrimTrailingHorizontalWhitespaceNative("a \t") === "a", "lite trim mismatch");
  assert(compressionCollapseNewlineRunsNative("a\n\n\nb") === "a\n\nb", "lite newline collapse mismatch");
  assert(compressionNormalizeMessageWhitespaceNative("a \n\n\nb\t") === "a\n\nb", "lite normalize mismatch");
  const trunc = compressionTruncateToolContentNative("hello wonderful world", 9);
  assert(trunc.startsWith("hello ") && trunc.endsWith("\n...[truncated]"), "lite truncate mismatch");
  console.log("compression_output_lite_ok");

  // Compression lite body transforms (W21+, opt-in)
  const collapsed = compressionCollapseWhitespaceNative({
    messages: [
      { role: "system", content: "a \n\n\nb\t" },
      { role: "user", content: "x \n\n\ny\t" },
    ],
  }, true);
  assert(collapsed.applied === true, "lite body collapse applied mismatch");
  assert(collapsed.body?.messages?.[0]?.content === "a \n\n\nb\t", "lite body collapse system preserved mismatch");
  assert(collapsed.body?.messages?.[1]?.content === "x\n\ny", "lite body collapse content mismatch");

  const deduped = compressionDedupSystemPromptNative({
    messages: [
      { role: "system", content: " same prompt " },
      { role: "user", content: "hi" },
      { role: "system", content: "same prompt" },
    ],
  });
  assert(deduped.applied === true, "lite body dedup applied mismatch");
  assert(deduped.body?.messages?.length === 2, "lite body dedup length mismatch");

  const compressedTools = compressionCompressToolResultsNative({
    messages: [{ role: "tool", content: "hello wonderful world ".repeat(200) }],
  });
  assert(compressedTools.applied === true, "lite body tool compress applied mismatch");
  assert(String(compressedTools.body?.messages?.[0]?.content).endsWith("\n...[truncated]"), "lite body tool compress content mismatch");

  const dedupContent = compressionRemoveRedundantContentNative({
    messages: [
      { role: "user", content: "same" },
      { role: "user", content: "same" },
      { role: "assistant", content: "ok" },
    ],
  });
  assert(dedupContent.applied === true, "lite body redundant applied mismatch");
  assert(dedupContent.body?.messages?.length === 2, "lite body redundant length mismatch");

  const replacedImgs = compressionReplaceImageUrlsNative(
    {
      messages: [
        {
          role: "user",
          content: [
            { type: "text", text: "look" },
            { type: "image_url", image_url: { url: "data:image/png;base64,abc" } },
          ],
        },
      ],
    },
    false
  );
  assert(replacedImgs.applied === true, "lite body image replace applied mismatch");
  assert(replacedImgs.body?.messages?.[0]?.content?.[1]?.type === "text", "lite body image replace type mismatch");
  assert(replacedImgs.body?.messages?.[0]?.content?.[1]?.text === "[image: png]", "lite body image replace text mismatch");
  console.log("compression_lite_body_ok");

  // Compression stats / token-estimation (W23+, opt-in)
  assert(compressionEstimateTokensNative("") === 0, "stats empty tokens mismatch");
  assert(compressionEstimateTokensNative("abcde") === 2, "stats token estimate mismatch");
  assert(compressionEstimateTokenSavingsNative("abcdefgh", "abcd") === 1, "stats savings mismatch");
  const ratio = compressionRatioNative("abcdefgh", "abcd");
  assert(Math.abs(ratio - 0.5) < 1e-9, "stats ratio mismatch");
  const savings = compressionSavingsNative("abcdefgh", "abcd");
  assert(savings.originalChars === 8 && savings.compressedChars === 4, "stats chars mismatch");
  assert(savings.savedTokensEst === 1, "stats saved tokens mismatch");
  console.log("compression_stats_ok");

  // Reasoning-field helpers (W25+, opt-in)
  assert(compressionHasNonEmptyReasoningContentNative({ reasoning_content: "hi" }) === true, "reasoning has mismatch");
  assert(compressionHasNonEmptyReasoningContentNative({ reasoning_content: "" }) === false, "reasoning empty mismatch");
  const ensured = compressionEnsureReasoningContentOnToolCallsNative([
    { role: "assistant", tool_calls: [{ id: "1" }], content: null },
  ]);
  assert(Array.isArray(ensured) && ensured[0].reasoning_content === "", "reasoning ensure mismatch");
  assert(compressionExtractReasoningContentNative({ reasoning_content: "hi" }) === "hi", "reasoning extract mismatch");
  assert(compressionExtractReasoningContentNative({}) === null, "reasoning extract null mismatch");
  const dropped = compressionDropReasoningContentNative({ reasoning_content: "x", content: "y" });
  assert(dropped.reasoning_content === undefined && dropped.content === "y", "reasoning drop mismatch");
  console.log("compression_reasoning_ok");

  console.log("CNV1_RUST_BRIDGE_SMOKE_OK");
}

try {
  main();
} catch (e) {
  console.error("CNV1_RUST_BRIDGE_SMOKE_FAIL", e instanceof Error ? e.message : String(e));
  process.exit(1);
}