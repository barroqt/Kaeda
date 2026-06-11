import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

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

  navigateRef.current = navigate;

  useEffect(() => {
    function handleKey(e) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        navigateRef.current(1);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        navigateRef.current(-1);
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
    </div>
  );
}
