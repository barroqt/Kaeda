import { useEffect, useRef } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";

export default function VideoPane({ videoPath }) {
  const videoRef = useRef(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video || !videoPath) return;

    const source = document.createElement("source");
    source.src = convertFileSrc(videoPath);
    source.type = "video/mp4";

    video.appendChild(source);
    video.load();

    return () => {
      while (video.firstChild) {
        video.removeChild(video.firstChild);
      }
    };
  }, [videoPath]);

  return (
    <div id="video-pane">
      <div id="video-container">
        {videoPath ? (
          <video ref={videoRef} controls />
        ) : (
          <div id="video-placeholder">No video file loaded</div>
        )}
      </div>
    </div>
  );
}
