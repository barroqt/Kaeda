import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import VideoPane from "./components/VideoPane";

const STORAGE_KEY = "kaeda-dark-mode";

function getInitialDark() {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored !== null) return stored === "true";
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export default function App() {
  const [subtitles, setSubtitles] = useState([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [dark, setDark] = useState(getInitialDark);
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
  const [toasts, setToasts] = useState([]);
  const navigateRef = useRef(null);
  const tokenNavRef = useRef(null);
  const saveRef = useRef(null);
  const markKnownRef = useRef(null);
  const toastIdRef = useRef(0);

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

  const fetchingLemmaRef = useRef(null);

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

  async function startSession() {
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
      await loadSubtitles();
      const name = await invoke("get_deck_name");
      setDeckName(name);
    } catch (err) {
      showToast(`Error: ${err}`, "error");
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
    if (selectedTokenIndex < 0) return;
    const current = subtitles[currentIndex];
    if (!current || !current.tokens || selectedTokenIndex >= current.tokens.length) return;
    const target = current.tokens[selectedTokenIndex].lemma;
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
    showToast("Audio replay coming soon", "info");
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

  useEffect(() => {
    function isInputFocused() {
      const tag = document.activeElement?.tagName;
      return tag === "INPUT" || tag === "TEXTAREA";
    }

    function handleKey(e) {
      if (isInputFocused()) return;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        navigateRef.current(1);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        navigateRef.current(-1);
      } else if (e.key === "ArrowLeft") {
        e.preventDefault();
        const ref = tokenNavRef.current;
        const tokens = ref.subtitles[ref.currentIndex]?.tokens;
        if (tokens && tokens.length > 0 && ref.selectedTokenIndex > 0) {
          ref.setSelectedTokenIndex(ref.selectedTokenIndex - 1);
        }
      } else if (e.key === "ArrowRight") {
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
      } else if (e.key === "s" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        navigateRef.current(1);
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
          <button onClick={startSession}>Start Session</button>
          <button onClick={() => setDark((d) => !d)}>
            {dark ? "Light" : "Dark"}
          </button>
        </div>
        <div id="subtitle-list">
          {subtitles.map((sub, i) => (
            <div
              key={sub.id}
              className={
                "subtitle-item" + (i === currentIndex ? " active" : "") + (sub.is_known ? " known" : "")
              }
              onClick={() => selectIndex(i)}
            >
              <div className="timestamp">
                {sub.start_time} &rarr; {sub.end_time}
              </div>
              <div className="text">{sub.text}</div>
            </div>
          ))}
        </div>
      </aside>
      <main id="main-panel" className={current ? "has-session" : ""}>
        {current ? (
          <>
            <VideoPane videoPath={videoPath} />
            <div id="current-subtitle">
              <div id="current-index">
                {currentIndex + 1} / {subtitles.length}
                {deckName && <span id="current-deck"> &mdash; Deck: {deckName}</span>}
              </div>
              <div id="current-timestamp">
                {current.start_time} &rarr; {current.end_time}
              </div>
              <div id="current-text">{current.text}</div>
            </div>
          </>
        ) : (
          <>
            <div id="current-subtitle">
              <div id="current-text">Start a session to begin mining</div>
            </div>
            <div id="help-text">
              <p>&uarr; &darr; Navigate subtitles</p>
              <p>&larr; &rarr; Select token</p>
              <p>&#8984;+Enter Save card</p>
              <p>k Mark line as known</p>
              <p>s Skip line</p>
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
                <div className="card-sentence">{current.text}</div>
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
                  <span className="action-hint">s</span>
                </div>
                <div className="action-group">
                  <button
                    className="replay-btn"
                    onClick={handleReplay}
                    title="Replay audio (coming soon)"
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
                    disabled={selectedTokenIndex < 0}
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
