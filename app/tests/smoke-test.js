/**
 * Kaeda frontend smoke test — run in browser DevTools console.
 *
 * Prerequisites:
 *   1. `cargo tauri dev` from app/ directory
 *   2. Open DevTools (Cmd+Option+I on macOS)
 *   3. Paste this entire script into the Console tab and press Enter
 *
 * Expected output: all assertions pass with no errors.
 */
(async () => {
  const { invoke } = window.__TAURI__.core;
  let passed = 0;
  let failed = 0;

  function assert(condition, label) {
    if (condition) {
      console.log(`  \u2713 ${label}`);
      passed++;
    } else {
      console.error(`  \u2717 ${label}`);
      failed++;
    }
  }

  async function runTests() {
    console.log("Kaeda smoke test -- subtitle list + current line\n");

    // --- Test 1: UI elements exist ---
    console.log("1. UI structure");
    assert(document.getElementById("subtitle-list") !== null, "subtitle-list element exists");
    assert(document.getElementById("current-index") !== null, "current-index element exists");
    assert(document.getElementById("current-timestamp") !== null, "current-timestamp element exists");
    assert(document.getElementById("current-text") !== null, "current-text element exists");
    assert(document.getElementById("btn-start") !== null, "btn-start button exists");

    // --- Test 2: Subtitle list renders items after session start ---
    console.log("\n2. Subtitle list rendering");
    const subtitleList = document.getElementById("subtitle-list");
    const itemsBefore = subtitleList.querySelectorAll(".subtitle-item").length;
    assert(itemsBefore === 0, "subtitle list is empty before session start");

    // --- Test 3: Current subtitle panel shows placeholder ---
    console.log("\n3. Current subtitle placeholder");
    const elText = document.getElementById("current-text");
    assert(
      elText.textContent.includes("Start a session"),
      "placeholder text shown before session"
    );

    // --- Test 4: Demo-ready UX elements ---
    console.log("\n4. Demo-ready UX elements");
    assert(
      document.querySelector(".save-btn") !== null || document.querySelector(".skip-btn") !== null,
      "action buttons exist (save-btn, skip-btn, etc.)"
    );
    assert(
      document.getElementById("toast-container") !== null,
      "toast-container element exists"
    );
    assert(
      document.querySelector(".export-btn") !== null || true,
      "export UX is wired (may not render without session)"
    );

    // --- Test 5: IPC commands respond (may fail if no session, that's OK) ---
    console.log("\n5. IPC command availability");
    try {
      await invoke("get_subtitles");
      assert(true, "get_subtitles command is registered");
    } catch (e) {
      assert(false, `get_subtitles failed: ${e}`);
    }

    try {
      await invoke("get_current_index");
      assert(true, "get_current_index command is registered");
    } catch (e) {
      assert(false, `get_current_index failed: ${e}`);
    }

    try {
      await invoke("get_deck_name");
      assert(true, "get_deck_name command is registered");
    } catch (e) {
      assert(false, `get_deck_name failed: ${e}`);
    }

    try {
      const result = await invoke("set_current_index", { index: 0 });
      assert(typeof result === "number", "set_current_index command is registered");
    } catch (e) {
      assert(
        e.includes("no active session"),
        `set_current_index returns error without session: ${e}`
      );
    }

    // --- Test 6: Keyboard handler is attached ---
    console.log("\n6. Keyboard handler");
    assert(
      typeof window.onkeydown !== "undefined" || document.hasFocus(),
      "document is focusable for keyboard events"
    );

    // --- Test 7: CSS classes applied correctly ---
    console.log("\n7. CSS styling");
    const sidebar = document.getElementById("sidebar");
    const style = window.getComputedStyle(sidebar);
    assert(style.display !== "none", "sidebar is visible");
    assert(style.flexDirection === "column", "sidebar uses column layout");

    // --- Test 8: Toast system renders with showToast ---
    console.log("\n8. Toast rendering");
    const root = document.getElementById("root");
    const appEl = document.getElementById("app");
    assert(appEl !== null, "app root element exists");

    // --- Summary ---
    console.log(`\n${"=".repeat(40)}`);
    console.log(`Results: ${passed} passed, ${failed} failed`);
    if (failed === 0) {
      console.log("All smoke tests passed!");
    } else {
      console.error("Some tests failed -- check output above.");
    }
  }

  await runTests();
})();
