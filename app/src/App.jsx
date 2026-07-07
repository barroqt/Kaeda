import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import VideoPane from "./components/VideoPane";

const STORAGE_KEY = "kaeda-dark-mode";

function formatMs(ms) {
  const totalSec = Math.floor(ms / 1000);
  const h = Math.floor(totalSec / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;
  const millis = ms % 1000;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")},${String(millis).padStart(3, "0")}`;
}

function getInitialDark() {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored !== null) return stored === "true";
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export default function App() {
  const [subtitles, setSubtitles] = useState([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [dark, setDark] = useState(getInitialDark);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsEnabled, setSettingsEnabled] = useState(false);
  const [settingsApiKey, setSettingsApiKey] = useState("");
  const [settingsTargetLang, setSettingsTargetLang] = useState("EN");
  const [settingsHasApiKey, setSettingsHasApiKey] = useState(false);
  // Contiguous token selection: { start, end } inclusive indices, or null.
  const [selectedRange, setSelectedRange] = useState(null);
  // Detected expression spans for the current subtitle (live lexicon).
  const [expressionSpans, setExpressionSpans] = useState([]);
  // Key of the span currently drilled into (Q/E), or null.
  const [expandedGroupKey, setExpandedGroupKey] = useState(null);
  // Learned expressions shown in the settings panel.
  const [learnedExpressions, setLearnedExpressions] = useState([]);
  const [explanation, setExplanation] = useState("");
  const [explanationLoading, setExplanationLoading] = useState(false);
  const [spanTranslation, setSpanTranslation] = useState("");
  const [spanTranslationLoading, setSpanTranslationLoading] = useState(false);
  const [spanTranslationError, setSpanTranslationError] = useState("");
  const [savedCard, setSavedCard] = useState(null);
  const [sessionCards, setSessionCards] = useState([]);
  const [viewingCards, setViewingCards] = useState(false);
  const [editingCard, setEditingCard] = useState(null);
  const [editSentence, setEditSentence] = useState("");
  const [editTarget, setEditTarget] = useState("");
  const [editExplanation, setEditExplanation] = useState("");
  const [decks, setDecks] = useState([]);
  const [activeDeckId, setActiveDeckId] = useState(null);
  const deckName = decks.find(d => d.id === activeDeckId)?.name || "";
  const [showDeckManager, setShowDeckManager] = useState(false);
  const [newDeckName, setNewDeckName] = useState("");
  const [renamingDeckId, setRenamingDeckId] = useState(null);
  const [renamingDeckName, setRenamingDeckName] = useState("");
  const [deletingDeckId, setDeletingDeckId] = useState(null);
  const [deletingDeckName, setDeletingDeckName] = useState("");
  const [videoPath, setVideoPath] = useState(null);
  const [sessionMode, setSessionMode] = useState(null);
  const [showNewSessionModal, setShowNewSessionModal] = useState(false);
  const [sessionError, setSessionError] = useState(null);
  const [toasts, setToasts] = useState([]);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState([]);
  const [searchHighlightIndex, setSearchHighlightIndex] = useState(-1);
  const searchTimer = useRef(null);
  const searchNavRef = useRef({ query: "", results: [], highlightIndex: -1 });
  const navigateRef = useRef(null);
  const tokenNavRef = useRef(null);
  const saveRef = useRef(null);
  const markKnownRef = useRef(null);
  const replayRef = useRef(null);
  const videoRef = useRef(null);
  const timeUpdateRef = useRef({ subtitles: [], currentIndex: 0, selectIndex: async () => {} });
  const replayTimeoutRef = useRef(null);
  const toastIdRef = useRef(0);
  const searchInputRef = useRef(null);
  // Fixed end of a shift+click span: set whenever the selection collapses
  // to a single token (click or plain A/D), never by shift interactions.
  const selectionAnchorRef = useRef(-1);

  function showToast(message, type = "info") {
    const id = ++toastIdRef.current;
    setToasts((prev) => [...prev, { id, message, type }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 4000);
  }

  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
    localStorage.setItem(STORAGE_KEY, String(dark));
  }, [dark]);

  async function loadDecks() {
    try {
      const [deckList, activeDeck] = await Promise.all([
        invoke("list_decks"),
        invoke("get_active_deck"),
      ]);
      setDecks(deckList);
      setActiveDeckId(activeDeck.id);
    } catch {
      /* no decks or store not initialized */
    }
  }

  const loadSubtitles = useCallback(async () => {
    try {
      const subs = await invoke("get_subtitles");
      const idx = await invoke("get_current_index");
      const vp = await invoke("get_video_path");
      setSubtitles(subs);
      setCurrentIndex(idx);
      setVideoPath(vp);
    } catch {
      /* no active session */
    }
    loadDecks();
  }, []);

  useEffect(() => {
    loadSubtitles();
  }, [loadSubtitles]);

  useEffect(() => {
    setSelectedRange(null);
    selectionAnchorRef.current = -1;
    setExpandedGroupKey(null);
    setExplanation("");
    setExplanationLoading(false);
    setSpanTranslation("");
    setSpanTranslationLoading(false);
    setSpanTranslationError("");
    setSavedCard(null);
    fetchingLemmaRef.current = null;
  }, [currentIndex]);

  const loadExpressionSpans = useCallback(async () => {
    try {
      const spans = await invoke("get_expression_spans", {
        subtitleIndex: currentIndex,
      });
      setExpressionSpans(spans);
    } catch {
      /* no active session */
      setExpressionSpans([]);
    }
  }, [currentIndex]);

  useEffect(() => {
    loadExpressionSpans();
  }, [loadExpressionSpans, subtitles]);

  // Drop a stale drill-down when its span disappears (e.g. the expression
  // was deleted from settings).
  useEffect(() => {
    if (
      expandedGroupKey &&
      !expressionSpans.some(
        (s) => `${s.start_index}-${s.end_index}` === expandedGroupKey,
      )
    ) {
      setExpandedGroupKey(null);
    }
  }, [expressionSpans, expandedGroupKey]);

  useEffect(() => {
    const el = document.querySelector(".subtitle-item.active");
    if (el) el.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }, [currentIndex]);

  useEffect(() => {
    const el = document.querySelector(".search-result-item.focused");
    if (el) el.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }, [searchHighlightIndex]);

  const fetchingLemmaRef = useRef(null);

  useEffect(() => {
    return () => {
      if (replayTimeoutRef.current) clearTimeout(replayTimeoutRef.current);
    };
  }, []);

  useEffect(() => {
    const unlisten = listen("translation-result", (event) => {
      const { lemma, translation: result, range } = event.payload;
      const pending = fetchingLemmaRef.current;
      // Single-lemma results match by lemma; expression results (which carry
      // a token range) match by range, since the frontend does not know the
      // dictionary form the backend keyed the event with.
      const matches =
        typeof pending === "string"
          ? range == null && pending === lemma
          : pending != null &&
            range != null &&
            pending.start === range[0] &&
            pending.end === range[1];
      if (!matches) return;
      setExplanationLoading(false);
      if (result != null) {
        setExplanation(result);
      } else {
        showToast("Dictionary lookup returned no result", "warning");
      }
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // Live re-detection (PRD §5.5): mining or deleting an expression mutates
  // the lexicon backend-side; refresh spans (and the settings list, when
  // open) so grouping appears/disappears without a reload.
  const expressionsChangedRef = useRef(() => {});
  useEffect(() => {
    const unlisten = listen("expressions-changed", () => {
      expressionsChangedRef.current();
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  useEffect(() => {
    if (searchTimer.current) clearTimeout(searchTimer.current);
    if (!searchQuery.trim()) {
      setSearchResults([]);
      return;
    }
    searchTimer.current = setTimeout(() => {
      invoke("search_subtitles", { query: searchQuery })
        .then((results) => setSearchResults(results))
        .catch((err) => {
          console.error("search_subtitles failed:", err);
          setSearchResults([]);
        });
    }, 250);
    return () => {
      if (searchTimer.current) clearTimeout(searchTimer.current);
    };
  }, [searchQuery]);

  useEffect(() => {
    setSearchHighlightIndex(-1);
  }, [searchResults]);

  useEffect(() => {
    if (!searchQuery.trim()) return;
    function handleClickOutside(e) {
      const container = e.target.closest(".search-results-container, #search-input, #search-bar-container");
      if (!container) {
        setSearchQuery("");
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [searchQuery]);

  useEffect(() => {
    if (!selectedRange) return;
    const current = subtitles[currentIndex];
    if (!current || !current.tokens || selectedRange.end >= current.tokens.length) return;
    const { start, end } = selectedRange;
    if (start === end) {
      const lemma = current.tokens[start].lemma;
      if (!lemma.trim()) return;
      fetchingLemmaRef.current = lemma;
      setExplanation("");
      setExplanationLoading(true);
      invoke("request_translation", { lemma })
        .then((result) => {
          if (fetchingLemmaRef.current !== lemma) return;
          if (result != null) {
            setExplanation(result);
            setExplanationLoading(false);
          }
        })
        .catch(() => {
          if (fetchingLemmaRef.current === lemma) {
            setExplanationLoading(false);
            showToast("Dictionary lookup failed", "error");
          }
        });
    } else {
      // The dictionary form (the event's lemma key) is computed backend-side,
      // so the pending marker is the range itself; the translation-result
      // listener correlates expression events by their range payload.
      const pending = { start, end };
      fetchingLemmaRef.current = pending;
      setExplanation("");
      setExplanationLoading(true);
      invoke("request_expression_translation", { startIndex: start, endIndex: end })
        .then((result) => {
          if (fetchingLemmaRef.current !== pending) return;
          if (result != null) {
            setExplanation(result);
            setExplanationLoading(false);
          }
        })
        .catch(() => {
          if (fetchingLemmaRef.current === pending) {
            setExplanationLoading(false);
            showToast("Expression lookup failed", "error");
          }
        });
    }
  }, [selectedRange, currentIndex, subtitles]);

  function openNewSessionModal() {
    setShowNewSessionModal(true);
  }

  async function startExternalSession() {
    setShowNewSessionModal(false);
    const srtPath = await open({
      multiple: false,
      filters: [{ name: "SRT subtitles", extensions: ["srt"] }],
    });
    if (!srtPath) return;

    const vidPath = await open({
      multiple: false,
      filters: [
        { name: "Video files", extensions: ["mp4", "mkv", "avi", "mov"] },
      ],
    });
    if (!vidPath) return;

    try {
      await invoke("start_session", {
        videoPath: vidPath,
        srtPath,
        deckName: "default",
      });
      setSessionCards([]);
      setViewingCards(false);
      setVideoPath(vidPath);
      setSessionMode("external");
      await loadSubtitles();
      await loadDecks();
    } catch (err) {
      setVideoPath(null);
      const msg =
        typeof err === "object" && err !== null
          ? err.message || String(err)
          : String(err);
      setSessionError(msg);
    }
  }

  async function startEmbeddedSession() {
    setShowNewSessionModal(false);
    const vidPath = await open({
      multiple: false,
      filters: [
        { name: "Video files", extensions: ["mp4", "mkv", "avi", "mov"] },
      ],
    });
    if (!vidPath) return;

    try {
      await invoke("start_embedded_session", {
        videoPath: vidPath,
        deckName: "default",
      });
      setSessionCards([]);
      setViewingCards(false);
      setVideoPath(vidPath);
      setSessionMode("embedded");
      await loadSubtitles();
      await loadDecks();
    } catch (err) {
      const msg =
        typeof err === "object" && err !== null
          ? err.message || String(err)
          : String(err);
      setSessionError(msg);
    }
  }

  async function selectIndex(index) {
    try {
      const idx = await invoke("set_current_index", { index });
      setCurrentIndex(idx);
    } catch {
      /* out of range */
    }
  }

  async function handleSubtitleClick(subtitleId) {
    const idx = subtitles.findIndex((s) => s.id === subtitleId);
    if (idx < 0) return;
    const sub = subtitles[idx];
    // v1 behavior: seek + pause — the user clicks to inspect a line, then manually plays.
    const video = videoRef.current;
    if (video) {
      video.currentTime = sub.start_ms / 1000.0;
      video.pause();
    }
    await selectIndex(idx);
  }

  const handleTimeUpdate = useCallback((timeSeconds) => {
    const { subtitles, currentIndex, selectIndex } = timeUpdateRef.current;
    if (!subtitles.length) return;
    const timeMs = timeSeconds * 1000;
    // Scan forward from currentIndex (most likely still playing), then wrap to 0
    for (let i = currentIndex; i < subtitles.length; i++) {
      const s = subtitles[i];
      if (s.start_ms <= timeMs && timeMs < s.end_ms) {
        if (i !== currentIndex) selectIndex(i);
        return;
      }
      if (s.start_ms > timeMs) break;
    }
    for (let i = 0; i < currentIndex; i++) {
      const s = subtitles[i];
      if (s.start_ms <= timeMs && timeMs < s.end_ms) {
        selectIndex(i);
        return;
      }
    }
  }, []);

  async function navigate(delta) {
    try {
      const idx = await invoke(
        delta > 0 ? "next_subtitle" : "previous_subtitle",
      );
      setCurrentIndex(idx);
    } catch {
      /* clamped or no session */
    }
  }

  async function handleSaveCard() {
    if (!explanation.trim()) return;
    const current = subtitles[currentIndex];
    if (!current) return;
    const validRange =
      selectedRange &&
      current.tokens &&
      selectedRange.end < current.tokens.length
        ? selectedRange
        : null;
    // For multi-token ranges the backend replaces the target with the
    // range's dictionary form and tags the card "expression".
    const target = validRange ? current.tokens[validRange.start].lemma : "";
    try {
      const card = await invoke("save_card", {
        target,
        explanation,
        tokenRange: validRange ? [validRange.start, validRange.end] : null,
      });
      setSavedCard(card);
      setExplanation("");
      showToast("Card saved", "success");
    } catch (err) {
      showToast(`Error saving card: ${err}`, "error");
    }
  }

  async function handleMarkKnown() {
    const current = subtitles[currentIndex];
    if (!current || current.is_known) return;
    try {
      await invoke("mark_line_known", { subtitleId: current.id });
      const subs = [...subtitles];
      subs[currentIndex] = { ...subs[currentIndex], is_known: true };
      setSubtitles(subs);
      showToast("Line marked as known", "success");
    } catch (err) {
      showToast(`Error: ${err}`, "error");
    }
  }

  async function handleDeckChange(newId) {
    try {
      await invoke("set_active_deck", { deckId: newId });
      setActiveDeckId(newId);
      showToast(`Switched to ${decks.find(d => d.id === newId)?.name || "deck"}`, "success");
    } catch (err) {
      showToast(`Error changing deck: ${err}`, "error");
    }
  }

  async function handleCreateDeck() {
    const name = newDeckName.trim();
    if (!name) return;
    try {
      await invoke("create_deck", { name });
      setNewDeckName("");
      setShowDeckManager(false);
      await loadDecks();
      showToast(`Created deck "${name}"`, "success");
    } catch (err) {
      showToast(`Failed to create deck: ${err}`, "error");
    }
  }

  function handleStartRename(id, name) {
    setRenamingDeckId(id);
    setRenamingDeckName(name);
  }

  async function handleSubmitRename() {
    const newName = renamingDeckName.trim();
    if (!newName) return;
    try {
      await invoke("rename_deck", { deckId: renamingDeckId, newName });
      setRenamingDeckId(null);
      setRenamingDeckName("");
      await loadDecks();
      showToast(`Renamed deck to "${newName}"`, "success");
    } catch (err) {
      showToast(`Failed to rename deck: ${err}`, "error");
    }
  }

  function handleCancelRename() {
    setRenamingDeckId(null);
    setRenamingDeckName("");
  }

  function handleStartDelete(id, name) {
    setDeletingDeckId(id);
    setDeletingDeckName(name);
  }

  async function handleConfirmDelete() {
    try {
      await invoke("delete_deck", { deckId: deletingDeckId });
      setDeletingDeckId(null);
      setDeletingDeckName("");
      await loadDecks();
      showToast("Deleted deck", "success");
    } catch (err) {
      showToast(`Failed to delete deck: ${err}`, "error");
    }
  }

  function handleCancelDelete() {
    setDeletingDeckId(null);
    setDeletingDeckName("");
  }

  async function handleSkip() {
    await navigate(1);
  }

  function handleReplay() {
    const sub = subtitles[currentIndex];
    if (!sub) return;
    const video = videoRef.current;
    if (!video) return;
    const startMs = Math.max(0, sub.start_ms - 200);
    const durationMs = (sub.end_ms - sub.start_ms) + 400;
    if (replayTimeoutRef.current) clearTimeout(replayTimeoutRef.current);
    video.currentTime = startMs / 1000.0;
    video.play();
    replayTimeoutRef.current = setTimeout(() => {
      video.pause();
      replayTimeoutRef.current = null;
    }, durationMs);
  }

  async function loadLearnedExpressions() {
    try {
      const expressions = await invoke("list_expressions");
      setLearnedExpressions(expressions);
    } catch {
      setLearnedExpressions([]);
    }
  }

  async function handleDeleteExpression(id) {
    try {
      // The backend emits expressions-changed, which re-detects spans so
      // grouping on the current subtitle vanishes immediately (PRD §5.7).
      await invoke("delete_expression", { id });
      await loadLearnedExpressions();
      showToast("Expression removed", "success");
    } catch (err) {
      const msg =
        typeof err === "object" && err !== null
          ? err.message || String(err)
          : String(err);
      showToast(`Failed to delete expression: ${msg}`, "error");
    }
  }

  async function openSettings() {
    try {
      const s = await invoke("get_translation_settings");
      setSettingsEnabled(s.enabled);
      setSettingsHasApiKey(s.has_api_key);
      setSettingsApiKey("");
      setSettingsTargetLang(s.target_lang);
      await loadLearnedExpressions();
      setSettingsOpen(true);
    } catch (err) {
      showToast(`Failed to load settings: ${err}`, "error");
    }
  }

  function closeSettings() {
    setSettingsOpen(false);
  }

  async function handleSaveSettings() {
    if (settingsEnabled && !settingsApiKey && !settingsHasApiKey) {
      showToast("API key is required to enable DeepL translation", "error");
      return;
    }
    try {
      await invoke("update_translation_settings", {
        newSettings: {
          enabled: settingsEnabled,
          api_key: settingsApiKey,
          target_lang: settingsTargetLang,
        },
      });
      setSettingsHasApiKey(settingsEnabled && (!!settingsApiKey || settingsHasApiKey));
      closeSettings();
      showToast("Settings saved", "success");
    } catch (err) {
      const msg = typeof err === "object" && err !== null ? err.message || String(err) : String(err);
      showToast(`Failed to save settings: ${msg}`, "error");
    }
  }

  async function loadSessionCards() {
    try {
      const cards = await invoke("get_session_cards");
      setSessionCards(cards);
    } catch {
      /* no active session */
    }
  }

  async function toggleViewCards() {
    if (!viewingCards) {
      await loadSessionCards();
    }
    setViewingCards((v) => !v);
  }

  async function handleExport() {
    const path = await save({
      filters: [{ name: "TSV files", extensions: ["tsv"] }],
      defaultPath: "kaeda-cards.tsv",
    });
    if (!path) return;
    try {
      await invoke("export_session", { path });
      showToast(`Exported to ${path}`, "success");
    } catch (err) {
      showToast(`Export failed: ${err}`, "error");
    }
  }

  function openEditDialog(card) {
    setEditingCard(card);
    setEditSentence(card.sentence);
    setEditTarget(card.target);
    setEditExplanation(card.explanation);
  }

  function closeEditDialog() {
    setEditingCard(null);
  }

  async function handleEditSave() {
    if (!editingCard) return;
    try {
      await invoke("edit_card", {
        cardId: editingCard.card_id,
        sentence: editSentence,
        target: editTarget,
        explanation: editExplanation,
      });
      closeEditDialog();
      await loadSessionCards();
      showToast("Card updated", "success");
    } catch (err) {
      showToast(`Error saving card: ${err}`, "error");
    }
  }

  async function handleDeleteCard(cardId) {
    if (!confirm("Delete this card?")) return;
    try {
      await invoke("delete_card", { cardId });
      closeEditDialog();
      await loadSessionCards();
      showToast("Card deleted", "success");
    } catch (err) {
      showToast(`Error deleting card: ${err}`, "error");
    }
  }

  /** Set the range, keeping the previous object when nothing changed so
   *  re-selecting the same token stays a no-op (as with the old index). */
  function applySelectedRange(next) {
    setSelectedRange((prev) =>
      prev && prev.start === next.start && prev.end === next.end ? prev : next,
    );
  }

  /** Collapse the selection to one token and reset the shift anchor. */
  function selectSingleToken(index) {
    selectionAnchorRef.current = index;
    applySelectedRange({ start: index, end: index });
  }

  function spanKey(span) {
    return `${span.start_index}-${span.end_index}`;
  }

  /** Detected span containing the token at `index`, or null. */
  function findSpanAt(index) {
    return (
      expressionSpans.find(
        (s) => index >= s.start_index && index <= s.end_index,
      ) || null
    );
  }

  /** Select a detected span as one unit (PRD §5.6). */
  function selectGroup(span) {
    selectionAnchorRef.current = span.start_index;
    applySelectedRange({ start: span.start_index, end: span.end_index });
  }

  /** Land on a token: a collapsed group underneath is selected as a whole,
   *  otherwise the single token is selected. */
  function landOnToken(index) {
    const span = findSpanAt(index);
    if (span && expandedGroupKey !== spanKey(span)) {
      selectGroup(span);
    } else {
      selectSingleToken(index);
    }
  }

  /** Drill into a group: individual morphemes become addressable. */
  function expandGroup(span, index = null) {
    setExpandedGroupKey(spanKey(span));
    selectSingleToken(index != null ? index : span.start_index);
  }

  /** Collapse the drill-down back to group-as-unit selection. */
  function collapseGroup() {
    if (!expandedGroupKey) return;
    const span =
      expressionSpans.find((s) => spanKey(s) === expandedGroupKey) || null;
    setExpandedGroupKey(null);
    if (span) selectGroup(span);
  }

  function handleTokenClick(index, shiftKey) {
    const anchor = selectionAnchorRef.current;
    if (shiftKey && selectedRange && anchor >= 0) {
      // Manual shift-selection works everywhere, including across groups.
      applySelectedRange({
        start: Math.min(anchor, index),
        end: Math.max(anchor, index),
      });
      return;
    }
    const span = findSpanAt(index);
    if (span && expandedGroupKey !== spanKey(span)) {
      if (
        selectedRange &&
        selectedRange.start === span.start_index &&
        selectedRange.end === span.end_index
      ) {
        // Clicking the selected group again drills down (PRD §5.6).
        expandGroup(span, index);
      } else {
        selectGroup(span);
      }
    } else {
      selectSingleToken(index);
    }
  }

  navigateRef.current = navigate;
  tokenNavRef.current = {
    selectedRange,
    subtitles,
    currentIndex,
    applySelectedRange,
    selectSingleToken,
    landOnToken,
    expressionSpans,
    expandedGroupKey,
    spanKey,
    expandGroup,
    collapseGroup,
  };
  expressionsChangedRef.current = () => {
    loadExpressionSpans();
    if (settingsOpen) loadLearnedExpressions();
  };
  saveRef.current = handleSaveCard;
  markKnownRef.current = handleMarkKnown;
  replayRef.current = handleReplay;
  timeUpdateRef.current = { subtitles, currentIndex, selectIndex };
  searchNavRef.current = { query: searchQuery, results: searchResults, highlightIndex: searchHighlightIndex, selectIndex };

  useEffect(() => {
    function isInputFocused() {
      const tag = document.activeElement?.tagName;
      return tag === "INPUT" || tag === "TEXTAREA";
    }

    function handleKey(e) {
      if (e.key === "Escape") {
        setSearchQuery("");
        document.activeElement?.blur();
        return;
      }
      if ((e.key === "f" || e.key === "F") && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        searchInputRef.current?.focus();
        return;
      }
      if (searchNavRef.current.query.trim() && searchNavRef.current.results.length > 0) {
        if (e.key === "ArrowDown" && !e.metaKey && !e.ctrlKey) {
          e.preventDefault();
          setSearchHighlightIndex(prev =>
            prev < searchNavRef.current.results.length - 1 ? prev + 1 : 0
          );
          return;
        }
        if (e.key === "ArrowUp" && !e.metaKey && !e.ctrlKey) {
          e.preventDefault();
          setSearchHighlightIndex(prev =>
            prev > 0 ? prev - 1 : searchNavRef.current.results.length - 1
          );
          return;
        }
        if (e.key === "Enter" && !e.metaKey && !e.ctrlKey) {
          e.preventDefault();
          const nav = searchNavRef.current;
          const idx = nav.highlightIndex;
          if (idx >= 0 && idx < nav.results.length) {
            nav.selectIndex(nav.results[idx].index);
            setSearchQuery("");
            document.activeElement?.blur();
          }
          return;
        }
      }
      if (isInputFocused()) return;
      if (e.key === "w" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        navigateRef.current(-1);
      } else if (e.key === "s" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        navigateRef.current(1);
      } else if (e.key === "q" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        // Q drills into the selected (collapsed) group (PRD §5.6
        // drill-down, rebound from S so W/S stay subtitle-only).
        const ref = tokenNavRef.current;
        const range = ref.selectedRange;
        const span = range
          ? ref.expressionSpans.find(
              (s) => s.start_index === range.start && s.end_index === range.end,
            )
          : null;
        if (span && ref.expandedGroupKey !== ref.spanKey(span)) {
          ref.expandGroup(span);
        }
      } else if (e.key === "e" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        // E collapses an active drill-down back to group-as-unit.
        tokenNavRef.current.collapseGroup();
      } else if ((e.key === "a" || e.key === "A") && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const ref = tokenNavRef.current;
        const tokens = ref.subtitles[ref.currentIndex]?.tokens;
        const range = ref.selectedRange;
        if (tokens && tokens.length > 0 && range) {
          if (e.shiftKey) {
            if (range.end > range.start) {
              // Shrink from the right.
              ref.applySelectedRange({ start: range.start, end: range.end - 1 });
            } else if (range.start > 0) {
              // Single token: extend one to the left.
              ref.applySelectedRange({ start: range.start - 1, end: range.end });
            }
          } else if (range.start > 0) {
            ref.landOnToken(range.start - 1);
          }
        }
      } else if ((e.key === "d" || e.key === "D") && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const ref = tokenNavRef.current;
        const tokens = ref.subtitles[ref.currentIndex]?.tokens;
        const range = ref.selectedRange;
        if (tokens && tokens.length > 0) {
          if (e.shiftKey) {
            if (range && range.end < tokens.length - 1) {
              // Grow the right edge.
              ref.applySelectedRange({ start: range.start, end: range.end + 1 });
            }
          } else if (!range) {
            ref.landOnToken(0);
          } else if (range.end < tokens.length - 1) {
            ref.landOnToken(range.end + 1);
          }
        }
      } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        saveRef.current();
      } else if (e.key === "k" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        markKnownRef.current();
      } else if (e.key === "r" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        replayRef.current();
      } else if (e.key === " " && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const video = videoRef.current;
        if (video) {
          video.paused ? video.play() : video.pause();
        }
      } else if (e.key === "ArrowLeft" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const video = videoRef.current;
        if (video) {
          video.currentTime = Math.max(0, video.currentTime - 15);
        }
      } else if (e.key === "ArrowRight" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const video = videoRef.current;
        if (video) {
          video.currentTime = Math.min(video.duration || 0, video.currentTime + 15);
        }
      } else if (e.key === "ArrowUp" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const video = videoRef.current;
        if (video) {
          video.volume = Math.min(1, (video.volume || 0) + 0.1);
        }
      } else if (e.key === "ArrowDown" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const video = videoRef.current;
        if (video) {
          video.volume = Math.max(0, (video.volume || 0) - 0.1);
        }
      } else if (e.key === "m" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const video = videoRef.current;
        if (video) {
          video.muted = !video.muted;
        }
      }
    }
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, []);

  const current = subtitles[currentIndex];

  // Render-time expression context (PRD §5.6): a detected group is first
  // selected as one highlighted unit; word-by-word access only via drill-down.
  const selectedExpressionSpan =
    selectedRange
      ? expressionSpans.find(
          (s) =>
            s.start_index === selectedRange.start &&
            s.end_index === selectedRange.end,
        ) || null
      : null;
  const expandedExpressionSpan = expandedGroupKey
    ? expressionSpans.find((s) => spanKey(s) === expandedGroupKey) || null
    : null;
  const canExpandExpression =
    selectedExpressionSpan &&
    expandedGroupKey !== spanKey(selectedExpressionSpan);

  return (
    <div id="app" className={sidebarCollapsed ? "sidebar-collapsed" : ""}>
      {sidebarCollapsed && (
        <button
          id="sidebar-expand-btn"
          title="Show subtitle panel"
          onClick={() => setSidebarCollapsed(false)}
        >
          &raquo;
        </button>
      )}
      <aside id="sidebar" className={sidebarCollapsed ? "collapsed" : ""}>
        <div id="toolbar">
          <button onClick={openNewSessionModal}>New Session</button>
          <button onClick={() => setDark((d) => !d)}>
            {dark ? "Light" : "Dark"}
          </button>
          <button onClick={openSettings}>Settings</button>
          <button
            id="sidebar-collapse-btn"
            title="Hide subtitle panel"
            onClick={() => setSidebarCollapsed(true)}
          >
            &laquo;
          </button>
        </div>
        {current && (
          <div id="session-info">
            <span id="session-progress">{currentIndex + 1} / {subtitles.length}</span>
            {deckName && <span id="session-deck">{deckName}</span>}
            {sessionMode && <span id="session-source">{sessionMode === "external" ? "SRT" : "Embedded"}</span>}
          </div>
        )}
        {current && (
          <div id="search-bar-container">
            <input
              ref={searchInputRef}
              id="search-input"
              type="text"
              placeholder="Search in subtitles…"
              autoComplete="off"
              autoCorrect="off"
              spellCheck="false"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  setSearchQuery("");
                  e.target.blur();
                }
              }}
            />
          </div>
        )}
        {searchQuery.trim() && (
          <div className="search-results-container">
            {searchResults.length > 0 ? (
              <>
                <div className="search-results-header">
                  검색 결과 ({searchResults.length}) / Results ({searchResults.length})
                </div>
                {searchResults.map((r, i) => (
                  <div
                    key={r.subtitle_id}
                    className={"search-result-item" + (r.index === currentIndex ? " active" : "") + (i === searchHighlightIndex ? " focused" : "")}
                    onClick={() => { selectIndex(r.index); setSearchQuery(""); }}
                  >
                    <div className="search-result-timestamp">{formatMs(r.start_ms)}</div>
                    <div className="search-result-line">{r.text}</div>
                  </div>
                ))}
              </>
            ) : (
              <div className="search-results-empty">No matches</div>
            )}
          </div>
        )}
        <div id="subtitle-list">
          {subtitles.map((sub, i) => (
            <div
              key={sub.id}
              className={
                "subtitle-item" + (i === currentIndex ? " active" : "") + (sub.is_known ? " known" : "")
              }
              onClick={() => handleSubtitleClick(sub.id)}
            >
              <div className="timestamp">
                {sub.start_time} &rarr; {sub.end_time}
              </div>
              <div className="text">{sub.text}</div>
            </div>
          ))}
        </div>
        {current && (
          <div id="sidebar-shortcuts">
            <span className="key">W</span><span className="key">S</span> prev / next line
            <span className="key">R</span> replay
            <span className="key">K</span> known
            <span className="shortcut-note">click a line to jump</span>
          </div>
        )}
      </aside>
      <main id="main-panel" className={current ? "has-session" : ""}>
        {current ? (
          <VideoPane ref={videoRef} videoPath={videoPath} onTimeUpdate={handleTimeUpdate} />
        ) : (
          <>
            <div id="current-subtitle">
              <div id="current-text">Start a session to begin mining</div>
            </div>
            <div id="help-text">
              <p className="help-group-title">Subtitles &mdash; left panel</p>
              <p><span className="key">W</span> <span className="key">S</span> Navigate subtitles</p>
              <p><span className="key">R</span> Replay current line</p>
              <p><span className="key">K</span> Mark line as known</p>
              <p>Click a subtitle to select it</p>
              <p className="help-group-title">Words &amp; card &mdash; right panel</p>
              <p><span className="key">A</span> <span className="key">D</span> Move word selection</p>
              <p><span className="key">Shift</span>+<span className="key">A</span>/<span className="key">D</span> Shrink / grow selection</p>
              <p><span className="key">Q</span> Open expression &middot; <span className="key">E</span> close it</p>
              <p><span className="key">&#8984;</span>+<span className="key">Enter</span> Save card</p>
            </div>
          </>
        )}
      </main>
      {current && (
        <aside id="right-panel">
          <button className="view-toggle" onClick={toggleViewCards}>
            {viewingCards ? "Back to Mining" : "View Cards"}
          </button>
          <div id="right-panel-header">
            <h2>{viewingCards ? "Session Cards" : "New Card"}</h2>
          </div>
          <div id="deck-selector">
            <label htmlFor="deck-select">Deck</label>
            <select
              id="deck-select"
              value={activeDeckId ?? ""}
              onChange={(e) => handleDeckChange(Number(e.target.value))}
            >
              {decks.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name}
                </option>
              ))}
            </select>
          </div>

          {viewingCards ? (
            <>
              <div id="session-cards-list">
                {(() => {
                  const filtered = sessionCards.filter((c) => c.deck_id === activeDeckId);
                  if (filtered.length === 0) {
                    return <div className="empty-cards">No cards in this deck.</div>;
                  }
                  return filtered.map((card, i) => (
                    <div key={card.card_id} className="session-card-item" onClick={() => openEditDialog(card)}>
                      <div className="session-card-index">#{i + 1}</div>
                      <div className="session-card-target">{card.target}</div>
                      <div className="session-card-sentence">{card.sentence}</div>
                      <div className="session-card-explanation">
                        {card.explanation || "\u2014"}
                      </div>
                    </div>
                  ));
                })()}
                <button className="export-btn" onClick={handleExport}>
                  Export deck '{deckName}' as TSV
                </button>
              </div>
            </>
          ) : (
            <>
              <div className="card-field">
                <label>Sentence</label>
                <div className="card-sentence-row">
                  <div className="card-sentence">{current.text}</div>
                  <div className="card-sentence-actions">
                    <button
                      className="action-icon-btn"
                      onClick={async () => {
                        try {
                          await invoke("copy_translation_span");
                          showToast("Sentence span copied", "success");
                        } catch (err) {
                          showToast(`Error: ${err}`, "error");
                        }
                      }}
                      title="Copy translation span to clipboard"
                    >
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                        <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                      </svg>
                    </button>
                    <button
                      className="action-icon-btn"
                      onClick={async () => {
                        setSpanTranslationLoading(true);
                        setSpanTranslation("");
                        setSpanTranslationError("");
                        try {
                          const result = await invoke("translate_current_span");
                          setSpanTranslation(result);
                        } catch (err) {
                          const code = err?.code || "";
                          const msg  = err?.message || String(err);
                          if (code === "TRANSLATION_DISABLED") {
                            setSpanTranslationError("Translation is not configured (see Settings).");
                          } else {
                            console.error(`[translate_current_span] ${code}: ${msg}`);
                            setSpanTranslationError(`Translation failed (${code}): ${msg}`);
                          }
                        } finally {
                          setSpanTranslationLoading(false);
                        }
                      }}
                      title="Translate current span via DeepL"
                    >
                      {spanTranslationLoading ? "Translating\u2026" : "Translate"}
                    </button>
                  </div>
                </div>
                {spanTranslation && (
                  <div className="span-translation">
                    <span className="span-translation-label">Translation:</span>
                    <span>{spanTranslation}</span>
                    <button
                      className="copy-to-card-btn"
                      onClick={() => setExplanation(spanTranslation)}
                    >
                      Copy to Card
                    </button>
                  </div>
                )}
                {spanTranslationError && (
                  <div className="span-translation span-translation-error">
                    {spanTranslationError}
                  </div>
                )}
              </div>

              <div className="card-field">
                <label>Target Word</label>
                <div className="word-tokens">
                  {current.tokens && current.tokens.length > 0 ? (
                    current.tokens.map((t, i) => {
                      const span = findSpanAt(i);
                      const expanded = span && expandedGroupKey === spanKey(span);
                      const className =
                        "word-token" +
                        (selectedRange &&
                        i >= selectedRange.start &&
                        i <= selectedRange.end
                          ? " selected"
                          : "") +
                        (span ? " in-expression" : "") +
                        (expanded ? " expression-expanded" : "") +
                        (span && i === span.start_index
                          ? " expression-start"
                          : "") +
                        (span && i === span.end_index ? " expression-end" : "");
                      return (
                        <span
                          key={i}
                          className={className}
                          onClick={(e) => handleTokenClick(i, e.shiftKey)}
                          title={
                            span && !expanded
                              ? span.display_form
                              : `${t.lemma} (${t.pos})`
                          }
                        >
                          {t.surface}
                        </span>
                      );
                    })
                  ) : (
                    <span className="word-token-empty">
                      No tokens available
                    </span>
                  )}
                </div>
                {canExpandExpression ? (
                  <div className="expression-hint">
                    Expression: {selectedExpressionSpan.display_form} &mdash;
                    press <span className="key">Q</span> (or click again) to
                    inspect word by word
                  </div>
                ) : expandedExpressionSpan ? (
                  <div className="expression-hint">
                    Inside expression &mdash; <span className="key">A</span>
                    <span className="key">D</span> move word by word &middot;{" "}
                    <span className="key">E</span> returns to the whole
                    expression
                  </div>
                ) : null}
              </div>

              <div className="card-field">
                <label>Translation</label>
                <textarea
                  value={explanation}
                  onChange={(e) => setExplanation(e.target.value)}
                  placeholder={explanationLoading ? "Loading translation..." : "Enter translation..."}
                  rows={4}
                />
              </div>

              <div className="action-row">
                <div className="action-group">
                  <button
                    className="skip-btn"
                    onClick={handleSkip}
                    title="Skip to next line"
                  >
                    Skip
                  </button>
                </div>
                <div className="action-group">
                  <button
                    className="replay-btn"
                    onClick={handleReplay}
                    title="Replay current line [r]"
                  >
                    Replay
                  </button>
                </div>
              </div>

              <div className="action-row">
                <div className="action-group">
                  <button
                    className="known-btn"
                    onClick={handleMarkKnown}
                    disabled={current.is_known}
                  >
                    {current.is_known ? "Known \u2713" : "Mark as Known"}
                  </button>
                  <span className="action-hint">k</span>
                </div>
                <div className="action-group">
                  <button
                    className="save-btn"
                    onClick={handleSaveCard}
                    disabled={!explanation.trim()}
                  >
                    Save Card
                  </button>
                  <span className="action-hint">&#8984;+Enter</span>
                </div>
              </div>

              {savedCard && (
                <div className="saved-notice">
                  Card saved: {savedCard.target} &mdash;{" "}
                  {savedCard.explanation}
                </div>
              )}
            </>
          )}
          <button className="manage-decks-btn" onClick={() => setShowDeckManager(true)}>
            Manage decks…
          </button>
          <div id="right-panel-shortcuts">
            <span className="key">A</span><span className="key">D</span> word
            <span className="key">&#8679;A</span><span className="key">&#8679;D</span> range
            <span className="shortcut-note">&#8679;click span</span>
            <span className={"key" + (canExpandExpression ? " key-active" : "")}>Q</span> open expr
            <span className={"key" + (expandedExpressionSpan ? " key-active" : "")}>E</span> close
            <span className="key">&#8984;</span>+<span className="key">Enter</span> save
          </div>
        </aside>
      )}

      {editingCard && (
        <div className="dialog-overlay" onClick={closeEditDialog}>
          <div className="dialog" onClick={(e) => e.stopPropagation()}>
            <h3>Edit Card</h3>

            <div className="dialog-field">
              <label>Sentence</label>
              <textarea
                value={editSentence}
                onChange={(e) => setEditSentence(e.target.value)}
                rows={3}
              />
            </div>

            <div className="dialog-field">
              <label>Target Word</label>
              <input
                type="text"
                value={editTarget}
                onChange={(e) => setEditTarget(e.target.value)}
              />
            </div>

            <div className="dialog-field">
              <label>Explanation</label>
              <textarea
                value={editExplanation}
                onChange={(e) => setEditExplanation(e.target.value)}
                rows={4}
              />
            </div>

            <div className="dialog-actions">
              <button className="dialog-btn dialog-btn-save" onClick={handleEditSave}>
                Save
              </button>
              <button className="dialog-btn dialog-btn-delete" onClick={() => handleDeleteCard(editingCard.card_id)}>
                Delete
              </button>
              <button className="dialog-btn dialog-btn-cancel" onClick={closeEditDialog}>
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {sessionError && (
        <div className="dialog-overlay" onClick={() => setSessionError(null)}>
          <div className="dialog session-error-dialog" onClick={(e) => e.stopPropagation()}>
            <h3>Session Error</h3>
            <p className="dialog-message">{sessionError}</p>
            <button className="dialog-btn dialog-btn-cancel" onClick={() => setSessionError(null)}>
              Dismiss
            </button>
          </div>
        </div>
      )}

      {showNewSessionModal && (
        <div className="dialog-overlay" onClick={() => setShowNewSessionModal(false)}>
          <div className="dialog new-session-dialog" onClick={(e) => e.stopPropagation()}>
            <h3>New Session</h3>
            <p className="dialog-subtitle">Choose subtitle source</p>

            <div className="session-mode-option" onClick={startExternalSession}>
              <div className="session-mode-text">
                <strong>Use external SRT</strong>
                <span className="session-mode-desc">Select an SRT file and a video file</span>
              </div>
            </div>

            <div className="session-mode-option" onClick={startEmbeddedSession}>
              <div className="session-mode-text">
                <strong>Use subtitles embedded in video</strong>
                <span className="session-mode-desc">Select a video file only</span>
              </div>
            </div>

            <button className="dialog-btn dialog-btn-cancel" onClick={() => setShowNewSessionModal(false)}>
              Cancel
            </button>
          </div>
        </div>
      )}

      {settingsOpen && (
        <div className="dialog-overlay" onClick={closeSettings}>
          <div className="dialog settings-dialog" onClick={(e) => e.stopPropagation()}>
            <h3>Translation Settings</h3>

            <div className="dialog-field">
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={settingsEnabled}
                  onChange={(e) => setSettingsEnabled(e.target.checked)}
                />
                Enable sentence translation with DeepL
              </label>
            </div>

            {settingsEnabled && (
              <>
                <div className="dialog-field">
                  <label>DeepL API Key</label>
                  <input
                    type="password"
                    value={settingsApiKey}
                    onChange={(e) => setSettingsApiKey(e.target.value)}
                    placeholder={settingsHasApiKey ? "Configured" : "Enter your DeepL API key"}
                    autoComplete="off"
                  />
                </div>

                <div className="dialog-field">
                  <label>Target Language</label>
                  <select
                    value={settingsTargetLang}
                    onChange={(e) => setSettingsTargetLang(e.target.value)}
                  >
                    <option value="EN">English (EN)</option>
                    <option value="DE">German (DE)</option>
                    <option value="FR">French (FR)</option>
                    <option value="ES">Spanish (ES)</option>
                    <option value="IT">Italian (IT)</option>
                    <option value="PT">Portuguese (PT)</option>
                    <option value="NL">Dutch (NL)</option>
                    <option value="PL">Polish (PL)</option>
                    <option value="RU">Russian (RU)</option>
                    <option value="JA">Japanese (JA)</option>
                    <option value="ZH">Chinese (ZH)</option>
                  </select>
                </div>
              </>
            )}

            <div className="dialog-field">
              <label>Learned expressions</label>
              {learnedExpressions.length === 0 ? (
                <div className="learned-expressions-empty">
                  No learned expressions yet. Mine a multi-token selection to
                  add one.
                </div>
              ) : (
                <div className="learned-expressions-list">
                  {learnedExpressions.map((expr) => (
                    <div key={expr.id} className="learned-expression-row">
                      <span className="learned-expression-form">
                        {expr.display_form}
                      </span>
                      <span className="learned-expression-date">
                        {expr.added_at.slice(0, 10)}
                      </span>
                      <button
                        className="deck-manager-icon-btn deck-manager-icon-delete"
                        onClick={() => handleDeleteExpression(expr.id)}
                        title="Delete expression"
                      >
                        &#10005;
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </div>

            <div className="dialog-actions">
              <button className="dialog-btn dialog-btn-save" onClick={handleSaveSettings}>
                Save
              </button>
              <button className="dialog-btn dialog-btn-cancel" onClick={closeSettings}>
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {showDeckManager && (
        <div className="dialog-overlay" onClick={() => setShowDeckManager(false)}>
          <div className="dialog deck-manager-dialog" onClick={(e) => e.stopPropagation()}>
            <h3>Manage Decks</h3>

            <div className="deck-manager-new">
              <input
                type="text"
                value={newDeckName}
                onChange={(e) => setNewDeckName(e.target.value)}
                placeholder="New deck name"
                onKeyDown={(e) => { if (e.key === "Enter") handleCreateDeck(); }}
              />
              <button className="deck-manager-new-btn" onClick={handleCreateDeck}>
                New Deck
              </button>
            </div>

            <div className="deck-manager-list">
              {decks.map((d) => {
                const cardCount = sessionCards.filter((c) => c.deck_id === d.id).length;
                return (
                  <div key={d.id} className="deck-manager-row">
                    {renamingDeckId === d.id ? (
                      <>
                        <input
                          type="text"
                          value={renamingDeckName}
                          onChange={(e) => setRenamingDeckName(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") handleSubmitRename();
                            if (e.key === "Escape") handleCancelRename();
                          }}
                          autoFocus
                        />
                        <button className="deck-manager-icon-btn" onClick={handleSubmitRename} title="Save">
                          &#10003;
                        </button>
                        <button className="deck-manager-icon-btn" onClick={handleCancelRename} title="Cancel">
                          &#10005;
                        </button>
                      </>
                    ) : (
                      <>
                        <span className="deck-manager-name">{d.name}</span>
                        <span className="deck-manager-count">{cardCount} card{cardCount !== 1 ? "s" : ""}</span>
                        <button className="deck-manager-icon-btn" onClick={() => handleStartRename(d.id, d.name)} title="Rename">
                          &#9998;
                        </button>
                        <button
                          className="deck-manager-icon-btn deck-manager-icon-delete"
                          onClick={() => handleStartDelete(d.id, d.name)}
                          title="Delete"
                          disabled={decks.length <= 1}
                        >
                          &#10005;
                        </button>
                      </>
                    )}
                  </div>
                );
              })}
            </div>

            <div className="dialog-actions">
              <button className="dialog-btn dialog-btn-cancel" onClick={() => setShowDeckManager(false)}>
                Close
              </button>
            </div>
          </div>
        </div>
      )}

      {deletingDeckId != null && (
        <div className="dialog-overlay" onClick={handleCancelDelete}>
          <div className="dialog confirm-dialog" onClick={(e) => e.stopPropagation()}>
            <h3>Delete Deck</h3>
            <p>
              Delete deck "{deletingDeckName}" and all its cards? This cannot be
              undone.
            </p>
            <div className="dialog-actions">
              <button className="dialog-btn dialog-btn-delete" onClick={handleConfirmDelete}>
                Delete
              </button>
              <button className="dialog-btn dialog-btn-cancel" onClick={handleCancelDelete}>
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      <div id="toast-container">
        {toasts.map((t) => (
          <div key={t.id} className={`toast toast-${t.type}`}>
            {t.message}
          </div>
        ))}
      </div>
    </div>
  );
}
