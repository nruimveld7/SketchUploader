import React from "react";
import { invoke } from "@tauri-apps/api/core";

export default function App() {
    const [msg, setMsg] = React.useState<string>("");

    async function callBackend() {
        // Calls the Rust command named "hello_world"
        const response = await invoke<string>("hello_world");
        setMsg(response);
    }

    return (
        <div style={{ fontFamily: "system-ui, sans-serif", padding: 24 }}>
            <h1>SketchUploader — Hello World demo</h1>
            <button onClick={callBackend} style={{ padding: "8px 14px", fontSize: 16 }}>
                Call Rust
            </button>
            <p style={{ marginTop: 16 }}>
                Result: {msg || <em>(click the button)</em>}
            </p>
        </div>
    );
}
