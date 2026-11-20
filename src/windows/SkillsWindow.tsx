import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import fraccionalIsotipo from "../assets/fraccional-isotipo.svg";
import { ERROR_TEXT } from "../core";

type Notice = { kind: "success" | "info" | "error"; text: string } | null;

function formatInstallCommand(command: string) {
  return command
    .replace("npx -y skills add ", "npx -y skills add \\\n  ")
    .replace(" -g -y -s ", " \\\n  -g -y \\\n  -s ");
}

/**
 * The skills window: a ZIP download of the granted services' skills, and the `npx skills add`
 * command for the core service skills (the same command as in the install prompt).
 *
 * @example
 * <SkillsWindow />
 */
export default function SkillsWindow() {
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<Notice>(null);
  const [downloadNotice, setDownloadNotice] = useState<string | null>(null);
  const [commandCopied, setCommandCopied] = useState(false);
  const [installCommand, setInstallCommand] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>("skills_install_command")
      .then(setInstallCommand)
      .catch((error) => setNotice({ kind: "error", text: ERROR_TEXT(error) }));
  }, []);

  async function downloadSkills() {
    setNotice(null);
    setDownloadNotice(null);
    const path = await save({
      defaultPath: "fraccional-googleworkspace-mcp-skills.zip",
      filters: [{ name: "ZIP", extensions: ["zip"] }],
    });
    if (path === null) {
      return;
    }
    setBusy(true);
    try {
      await invoke("export_skills_zip", { path });
      setDownloadNotice("Skills descargadas correctamente.");
    } catch (error) {
      setNotice({ kind: "error", text: ERROR_TEXT(error) });
    } finally {
      setBusy(false);
    }
  }

  async function copyCommand() {
    if (installCommand === null) {
      return;
    }
    try {
      await navigator.clipboard.writeText(installCommand);
      setCommandCopied(true);
      window.setTimeout(() => setCommandCopied(false), 1800);
      setNotice({ kind: "success", text: "Comando copiado al portapapeles." });
    } catch (error) {
      setNotice({ kind: "error", text: ERROR_TEXT(error) });
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
          <h1 className="text-2xl font-semibold">Skills</h1>
        </header>

        <section className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900">
          <h2 className="font-semibold">Descargar skills</h2>
          <p className="mt-1 text-sm text-slate-600 dark:text-zinc-400">
            Descarga un ZIP con las skills correspondientes a los permisos
            configurados.
          </p>
          <div className="mt-4 flex items-center gap-2">
            <button
              type="button"
              disabled={busy}
              onClick={downloadSkills}
              className="rounded-lg bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-700 disabled:opacity-50 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-zinc-300"
            >
              {busy ? "Preparando ZIP…" : "Descargar .zip"}
            </button>
            {downloadNotice !== null && (
              <span className="text-xs text-emerald-700 dark:text-emerald-400">
                {downloadNotice}
              </span>
            )}
          </div>
        </section>

        <section className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-zinc-800 dark:bg-zinc-900">
          <h2 className="font-semibold">Avanzado</h2>
          <p className="mt-1 text-sm text-slate-600 dark:text-zinc-400">
            Instala las skills principales de Google Workspace con el gestor
            oficial.
          </p>
          {installCommand !== null && (
            <div className="mt-4 flex items-start gap-2 rounded-lg bg-slate-100 p-3 dark:bg-zinc-800">
              <pre className="min-w-0 flex-1 whitespace-pre-wrap break-words text-xs">
                {formatInstallCommand(installCommand)}
              </pre>
              <button
                type="button"
                onClick={copyCommand}
                className={`shrink-0 rounded-md border px-2 py-1 text-xs font-medium transition-colors ${
                  commandCopied
                    ? "border-emerald-500 bg-emerald-50 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300"
                    : "border-slate-300 hover:bg-white dark:border-zinc-600 dark:hover:bg-zinc-700"
                }`}
              >
                {commandCopied ? "Copiado" : "Copiar"}
              </button>
            </div>
          )}
        </section>

        {notice !== null && (
          <p
            role="status"
            className={
              notice.kind === "error"
                ? "text-sm text-red-600 dark:text-red-400"
                : "text-sm text-emerald-700 dark:text-emerald-400"
            }
          >
            {notice.text}
          </p>
        )}
      </div>
    </main>
  );
}
