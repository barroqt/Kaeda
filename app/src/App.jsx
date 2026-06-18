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
  const [selectedTokenIndex, setSelectedTokenIndex] = useState(-1);
  const [explanation, setExplanation] = useState("");
  const [explanationLoading, setExplanationLoading] = useState(false);
  const [savedCard, setSavedCard] = useState(null);
  const [sessionCards, setSessionCards] = useState([]);
  const [viewingCards, setViewingCards] = useState(false);
  const [editingCard, setEditingCard] = useState(null);
  const [editSentence, setEditSentence] = useState("");
  const [editTarget, setEditTarget] = useState("");
  const [editExplanation, setEditExplanation] = useState("");
  const [deckName, setDeckName] = useState("");
  const [videoPath, setVideoPath] = useState(null);
  const [sessionMode, setSessionMode] = useState(null);
  const [showNewSessionModal, setShowNewSessionModal] = useState(false);
  const [sessionError, setSessionError] = useState(null);
  const [toasts, setToasts] = useState([]);
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
  }, []);

  useEffect(() => {
    loadSubtitles();
  }, [loadSubtitles]);

  useEffect(() => {
    setSelectedTokenIndex(-1);
    setExplanation("");
    setExplanationLoading(false);
    setSavedCard(null);
    fetchingLemmaRef.current = null;
  }, [currentIndex]);

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
      const { lemma, translation: result } = event.payload;
      if (fetchingLemmaRef.current !== lemma) return;
      setExplanationLoading(false);
      if (result != null) {
        setExplanation(result);
      } else {
        showToast("Dictionary lookup returned no result", "warning");
      }
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
    if (selectedTokenIndex < 0) return;
    const current = subtitles[currentIndex];
    if (!current || !current.tokens || selectedTokenIndex >= current.tokens.length) return;
    const lemma = current.tokens[selectedTokenIndex].lemma;
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
  }, [selectedTokenIndex, currentIndex, subtitles]);

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
      const name = await invoke("get_deck_name");
      setDeckName(name);
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
      const name = await invoke("get_deck_name");
      setDeckName(name);
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
    const target =
      selectedTokenIndex >= 0 &&
      current.tokens &&
      selectedTokenIndex < current.tokens.length
        ? current.tokens[selectedTokenIndex].lemma
        : "";
    try {
      const card = await invoke("save_card", {
        target,
        explanation,
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

  async function openSettings() {
    try {
      const s = await invoke("get_translation_settings");
      setSettingsEnabled(s.enabled);
      setSettingsHasApiKey(s.has_api_key);
      setSettingsApiKey("");
      setSettingsTargetLang(s.target_lang);
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

  navigateRef.current = navigate;
  tokenNavRef.current = { selectedTokenIndex, subtitles, currentIndex, setSelectedTokenIndex };
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
      } else if (e.key === "a" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const ref = tokenNavRef.current;
        const tokens = ref.subtitles[ref.currentIndex]?.tokens;
        if (tokens && tokens.length > 0 && ref.selectedTokenIndex > 0) {
          ref.setSelectedTokenIndex(ref.selectedTokenIndex - 1);
        }
      } else if (e.key === "d" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        const ref = tokenNavRef.current;
        const tokens = ref.subtitles[ref.currentIndex]?.tokens;
        if (tokens && tokens.length > 0 && ref.selectedTokenIndex < tokens.length - 1) {
          ref.setSelectedTokenIndex(
            ref.selectedTokenIndex < 0 ? 0 : ref.selectedTokenIndex + 1,
          );
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

  return (
    <div id="app">
      <aside id="sidebar">
        <div id="toolbar">
          <button onClick={openNewSessionModal}>New Session</button>
          <button onClick={() => setDark((d) => !d)}>
            {dark ? "Light" : "Dark"}
          </button>
          <button onClick={openSettings}>Settings</button>
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
            <span className="key">W</span><span className="key">S</span> subs
            <span className="key">A</span><span className="key">D</span> word
            <span className="key">R</span> replay
            <span className="key">K</span> known
            <span className="key">&#8984;</span>+<span className="key">Enter</span> save
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
              <p><span className="key">W</span> <span className="key">S</span> Navigate subtitles</p>
              <p><span className="key">A</span> <span className="key">D</span> Select token</p>
              <p><span className="key">R</span> Replay current line</p>
              <p><span className="key">&#8984;</span>+<span className="key">Enter</span> Save card</p>
              <p><span className="key">K</span> Mark line as known</p>
              <p>Click a subtitle to select it</p>
            </div>
          </>
        )}
      </main>
      {current && (
        <aside id="right-panel">
          <div id="right-panel-header">
            <h2>{viewingCards ? "Session Cards" : "New Card"}</h2>
            <div id="right-panel-header-actions">
              {deckName && <span id="deck-label">{deckName}</span>}
              <button className="view-toggle" onClick={toggleViewCards}>
                {viewingCards ? "Back to Mining" : "View Cards"}
              </button>
            </div>
          </div>

          {viewingCards ? (
            <div id="session-cards-list">
              {sessionCards.length === 0 ? (
                <div className="empty-cards">No cards saved yet.</div>
              ) : (
                sessionCards.map((card, i) => (
                  <div key={card.card_id} className="session-card-item" onClick={() => openEditDialog(card)}>
                    <div className="session-card-index">#{i + 1}</div>
                    <div className="session-card-target">{card.target}</div>
                    <div className="session-card-sentence">{card.sentence}</div>
                    <div className="session-card-explanation">
                      {card.explanation || "\u2014"}
                    </div>
                  </div>
                ))
              )}
              <button className="export-btn" onClick={handleExport}>
                Export TSV
              </button>
            </div>
          ) : (
            <>
              <div className="card-field">
                <label>Sentence</label>
                <div className="card-sentence-row">
                  <div className="card-sentence">{current.text}</div>
                  <button
                    className="copy-span-btn"
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
                </div>
              </div>

              <div className="card-field">
                <label>Target Word</label>
                <div className="word-tokens">
                  {current.tokens && current.tokens.length > 0 ? (
                    current.tokens.map((t, i) => (
                      <span
                        key={i}
                        className={
                          "word-token" +
                          (selectedTokenIndex === i ? " selected" : "")
                        }
                        onClick={() => setSelectedTokenIndex(i)}
                        title={`${t.lemma} (${t.pos})`}
                      >
                        {t.surface}
                      </span>
                    ))
                  ) : (
                    <span className="word-token-empty">
                      No tokens available
                    </span>
                  )}
                </div>
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
