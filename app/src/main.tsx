import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./app/theme.css";
import { applyTheme, resolveInitialTheme } from "./app/useTheme";

// 先落主题再渲染，避免首帧闪一下白/黑
applyTheme(resolveInitialTheme());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
