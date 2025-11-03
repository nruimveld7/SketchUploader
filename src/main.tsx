import React from "react";
import { createRoot } from "react-dom/client";
import "./styles/tailwind.css";          // Tailwind styles get included here
import App from "./App";

createRoot(document.getElementById("root")!).render(<App />);