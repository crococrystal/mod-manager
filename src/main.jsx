import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { WindowFrame } from "./components/WindowFrame.jsx";

// Класс на <html> позволяет CSS подстроиться под конкретную ОС:
// на Windows DWM рисует своё системное скругление и тень, поэтому
// мы понижаем CSS border-radius до 8px, чтобы он совпал с системным
// и не возникало sub-pixel-артефактов на границах окна.
const ua = navigator.userAgent || "";
const platformClass = ua.includes("Windows")
  ? "platform-windows"
  : ua.includes("Mac OS X")
  ? "platform-macos"
  : ua.includes("Linux")
  ? "platform-linux"
  : null;
if (platformClass) {
  document.documentElement.classList.add(platformClass);
}

ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <WindowFrame>
      <App />
    </WindowFrame>
  </React.StrictMode>,
);
