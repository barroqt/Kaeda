import { forwardRef, useState, useCallback, useEffect } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";

const LOCALHOST = "127.0.0.1";

const VideoPane = forwardRef(({ videoPath }, ref) => {
  const [error, setError] = useState(null);
  const [url, setUrl] = useState("");

  useEffect(() => {
    setError(null);
    if (!videoPath) {
      setUrl("");
      return;
    }
    if (!isTauri()) {
      setUrl("");
      setError("Not running in Tauri — video unavailable");
      return;
    }
    (async () => {
      try {
        const port = await invoke("get_video_server_port");
        if (port === 0) {
          setError("Video server not available");
          return;
        }
        const u = `http://${LOCALHOST}:${port}/${encodeURIComponent(videoPath)}`;
        setUrl(u);
      } catch (e) {
        setError(`Failed to get video server port: ${e}`);
      }
    })();
  }, [videoPath]);

  const handleVideoError = useCallback((e) => {
    const video = e.currentTarget;
    const ve = video.error;
    const labels = ["", "ABORTED", "NETWORK", "DECODE", "SRC_NOT_SUPPORTED"];
    if (ve) {
      setError(`Video error ${ve.code} (${labels[ve.code] || "unknown"}): ${ve.message}`);
    } else {
      setError(`Video error (no MediaError object)`);
    }
  }, []);

  const handleSourceError = useCallback((e) => {
    const src = e.currentTarget;
    setError(`Source error loading: ${src.src ? src.src.substring(0, 80) : "none"}`);
  }, []);

  return (
    <div id="video-pane">
      <div id="video-container">
        {error && <div id="video-error">{error}</div>}
        {videoPath && url ? (
          <video ref={ref} id="kaeda-video" controls onError={handleVideoError}>
            <source key={url} src={url} onError={handleSourceError} />
          </video>
        ) : videoPath ? (
          <div id="video-placeholder">Preparing video…</div>
        ) : (
          <div id="video-placeholder">No video file loaded</div>
        )}
      </div>
    </div>
  );
});

export default VideoPane;
