import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

const STORAGE_KEY = "kaeda-dark-mode";

function getInitialDark() {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored !== null) return stored === "true";
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function splitWords(text) {
  return text.split(/(\s+)/).map((t, i) => ({
    text: t,
    key: i,
    isSpace: /^\s+$/.test(t),
  }));
}

export default function App() {
  const [subtitles, setSubtitles] = useState([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [dark, setDark] = useState(getInitialDark);
  const [selectedTarget, setSelectedTarget] = useState("");
  const [explanation, setExplanation] = useState("");
  const [savedCard, setSavedCard] = useState(null);
  const [sessionCards, setSessionCards] = useState([]);
  const [viewingCards, setViewingCards] = useState(false);
  const navigateRef = useRef(null);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
    localStorage.setItem(STORAGE_KEY, String(dark));
  }, [dark]);

  const loadSubtitles = useCallback(async () => {
    try {
      const subs = await invoke("get_subtitles");
      const idx = await invoke("get_current_index");
      setSubtitles(subs);
      setCurrentIndex(idx);
    } catch {
      /* no active session */
    }
  }, []);

  useEffect(() => {
    loadSubtitles();
  }, [loadSubtitles]);

  useEffect(() => {
    setSelectedTarget("");
    setExplanation("");
    setSavedCard(null);
  }, [currentIndex]);

  async function startSession() {
    const srtPath = await open({
      multiple: false,
      filters: [{ name: "SRT subtitles", extensions: ["srt"] }],
    });
    if (!srtPath) return;

    try {
      await invoke("start_session", {
        videoPath: srtPath,
        srtPath,
        deckName: "default",
      });
      setSessionCards([]);
      setViewingCards(false);
      await loadSubtitles();
    } catch (err) {
      alert(`Error: ${err}`);
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
    if (!selectedTarget.trim()) return;
    try {
      const card = await invoke("save_card", {
        target: selectedTarget.trim(),
        explanation,
      });
      setSavedCard(card);
      setExplanation("");
    } catch (err) {
      alert(`Error: ${err}`);
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

  navigateRef.current = navigate;

  useEffect(() => {
    function handleKey(e) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        navigateRef.current(1);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        navigateRef.current(-1);
      } else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        handleSaveCard();
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
                "subtitle-item" + (i === currentIndex ? " active" : "")
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
      <main id="main-panel">
        {current ? (
          <div id="current-subtitle">
            <div id="current-index">
              {currentIndex + 1} / {subtitles.length}
            </div>
            <div id="current-timestamp">
              {current.start_time} &rarr; {current.end_time}
            </div>
            <div id="current-text">{current.text}</div>
          </div>
        ) : (
          <>
            <div id="current-subtitle">
              <div id="current-text">Start a session to begin mining</div>
            </div>
            <div id="help-text">
              <p>&uarr; &darr; Navigate subtitles</p>
              <p>Click a subtitle to select it</p>
            </div>
          </>
        )}
      </main>
      {current && (
        <aside id="right-panel">
          <div id="right-panel-header">
            <h2>{viewingCards ? "Session Cards" : "New Card"}</h2>
            <button className="view-toggle" onClick={toggleViewCards}>
              {viewingCards ? "Back to Mining" : "View Cards"}
            </button>
          </div>

          {viewingCards ? (
            <div id="session-cards-list">
              {sessionCards.length === 0 ? (
                <div className="empty-cards">No cards saved yet.</div>
              ) : (
                sessionCards.map((card, i) => (
                  <div key={i} className="session-card-item">
                    <div className="session-card-index">#{i + 1}</div>
                    <div className="session-card-target">{card.target}</div>
                    <div className="session-card-sentence">{card.sentence}</div>
                    <div className="session-card-explanation">
                      {card.explanation || "\u2014"}
                    </div>
                  </div>
                ))
              )}
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
                  {splitWords(current.text).map((t) =>
                    t.isSpace ? (
                      <span key={t.key} className="word-space">
                        {" "}
                      </span>
                    ) : (
                      <span
                        key={t.key}
                        className={
                          "word-token" +
                          (selectedTarget === t.text ? " selected" : "")
                        }
                        onClick={() => setSelectedTarget(t.text)}
                      >
                        {t.text}
                      </span>
                    ),
                  )}
                </div>
              </div>

              <div className="card-field">
                <label>Explanation</label>
                <textarea
                  value={explanation}
                  onChange={(e) => setExplanation(e.target.value)}
                  placeholder="Enter explanation..."
                  rows={4}
                />
              </div>

              <button
                className="save-btn"
                onClick={handleSaveCard}
                disabled={!selectedTarget.trim()}
              >
                Save Card
              </button>

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
    </div>
  );
}
