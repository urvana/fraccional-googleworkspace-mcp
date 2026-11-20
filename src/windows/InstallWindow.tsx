import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import fraccionalIsotipo from "../assets/fraccional-isotipo.svg";
import { ERROR_TEXT, type InstallPrompt } from "../core";

type InstallState =
  | { kind: "idle" }
  | { kind: "running" }
  | { kind: "done" }
  | { kind: "error"; text: string };

/**
 * The state of the one-click install button in the install window.
 *
 * @example
 * const state: InstallState = { kind: "error", text: "No hay una app para abrirlo." };
 */

/**
 * The install window: a one-click MCP extension install, and the install prompt that a person
 * pastes into an agent. It rebuilds the prompt on mount and each time the tray shows the window
 * again, and clears the result of the one-click install.
 *
 * @example
 * <InstallWindow />
 */
export default function InstallWindow() {
  const [prompt, setPrompt] = useState<InstallPrompt | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [copyError, setCopyError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [installState, setInstallState] = useState<InstallState>({
    kind: "idle",
  });

  const latestRequest = useRef(0);

  useEffect(() => {
    async function loadPrompt() {
      latestRequest.current += 1;
      const request = latestRequest.current;
      try {
        const result = await invoke<InstallPrompt>("install_prompt");
        if (request === latestRequest.current) {
          setPrompt(result);
          setLoadError(null);
        }
      } catch (error) {
        if (request === latestRequest.current) {
          setPrompt(null);
          setLoadError(ERROR_TEXT(error));
        }
      }
    }

    function onShown() {
      setPrompt(null);
      setLoadError(null);
      setCopyError(null);
      setCopied(false);
      setInstallState({ kind: "idle" });
      loadPrompt();
    }

    loadPrompt();
    const unlisten = listen("install-window-shown", onShown);
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  async function copyPrompt() {
    if (prompt === null) {
      return;
    }
    try {
      await navigator.clipboard.writeText(prompt.text);
      setCopyError(null);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch (error) {
      setCopyError(ERROR_TEXT(error));
    }
  }

  async function installMcp() {
    setInstallState({ kind: "running" });
    try {
      await invoke<string | null>("install_mcp_extension");
      setInstallState({ kind: "done" });
    } catch (error) {
      setInstallState({ kind: "error", text: ERROR_TEXT(error) });
    }
  }

  return (
    <main className="min-h-screen bg-slate-50 p-6 text-slate-900 dark:bg-zinc-950 dark:text-zinc-100">
      <div className="mx-auto max-w-xl space-y-5">
        <header>
          <img
            src={fraccionalIsotipo}
            alt="Fraccional"
            className="h-9 w-9 rounded-lg bg-zinc-900 p-1.5"
          />
          <p className="mt-2 text-sm font-medium text-slate-500 dark:text-zinc-400">
            fraccional-googleworkspace-mcp
          </p>
          <h1 className="text-2xl font-semibold">Conectar un agente</h1>
        </header>

        {loadError !== null && (
          <p
            role="alert"
            className="rounded-2xl border border-red-200 bg-red-50 p-5 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
          >
            {loadError}
          </p>
        )}

        {prompt !== null && (
          <>
            {prompt.warning !== null && (
              <p
                role="status"
                className="rounded-2xl border border-amber-200 bg-amber-50 p-4 text-sm text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-300"
              >
                {prompt.warning}
              </p>
            )}

            <section className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900">
              <h2 className="font-semibold">Instalación en un clic</h2>
              <div className="mt-4 flex items-center gap-2">
                <button
                  type="button"
                  disabled={installState.kind === "running"}
                  onClick={installMcp}
                  className="rounded-lg bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-zinc-300"
                >
                  {installState.kind === "running"
                    ? "Abriendo…"
                    : "Instalar MCP"}
                </button>
                {installState.kind === "done" && (
                  <span
                    role="status"
                    className="text-xs text-emerald-700 dark:text-emerald-400"
                  >
                    Listo. Sigue los pasos en tu app de IA.
                  </span>
                )}
                {installState.kind === "error" && (
                  <span
                    role="alert"
                    className="text-xs text-red-600 dark:text-red-400"
                  >
                    {installState.text}
                  </span>
                )}
              </div>
              <p className="mt-2 text-sm text-slate-600 dark:text-zinc-400">
                Se abre tu app de IA (por ejemplo, Claude Desktop). Ahí pulsa
                "Instalar".
              </p>
            </section>

            <section className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900">
              <h2 className="font-semibold">
                Con un prompt (Claude Code, Codex y otros agentes)
              </h2>
              <ol className="mt-3 list-decimal space-y-1 pl-5 text-sm text-slate-600 dark:text-zinc-400">
                <li>Copia el prompt.</li>
                <li>
                  Pégalo en tu agente (Claude Code, Codex, Hermes, Cursor…) o en
                  ChatGPT de escritorio.
                </li>
                <li>
                  Sigue sus pasos. No funciona en claude.ai ni en chatgpt.com.
                </li>
              </ol>
              <pre className="mt-4 max-h-80 overflow-y-auto whitespace-pre-wrap rounded-lg bg-slate-100 p-3 font-mono text-xs dark:bg-zinc-800">
                {prompt.text}
              </pre>
              <div className="mt-4 flex items-center gap-2">
                <button
                  type="button"
                  onClick={copyPrompt}
                  className={`rounded-lg border px-4 py-2 text-sm font-medium transition-colors ${
                    copied
                      ? "border-emerald-500 bg-emerald-50 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300"
                      : "border-slate-300 hover:bg-slate-50 dark:border-zinc-600 dark:hover:bg-zinc-800"
                  }`}
                >
                  {copied ? "Copiado" : "Copiar"}
                </button>
                {copyError !== null && (
                  <span
                    role="alert"
                    className="text-xs text-red-600 dark:text-red-400"
                  >
                    {copyError}
                  </span>
                )}
              </div>
            </section>
          </>
        )}
      </div>
    </main>
  );
}
