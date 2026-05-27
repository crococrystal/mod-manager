import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { WindowFrame } from "./components/WindowFrame.jsx";

ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <WindowFrame>
      <App />
    </WindowFrame>
  </React.StrictMode>,
);
