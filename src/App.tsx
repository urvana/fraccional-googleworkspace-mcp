import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useEffect, useRef, useState } from "react";
import fraccionalIsotipo from "./assets/fraccional-isotipo.svg";
import adminIcon from "./assets/service-icons/admin.png";
import businessIntelligenceIcon from "./assets/service-icons/business-intelligence.png";
import calendarIcon from "./assets/service-icons/calendar.png";
import chatIcon from "./assets/service-icons/chat.png";
import dataAnalyticsIcon from "./assets/service-icons/data-analytics.png";
import docsIcon from "./assets/service-icons/docs.png";
import driveIcon from "./assets/service-icons/drive.png";
import formsIcon from "./assets/service-icons/forms.png";
import gmailIcon from "./assets/service-icons/gmail.png";
import mapsIcon from "./assets/service-icons/maps.png";
import marketplaceIcon from "./assets/service-icons/marketplace.png";
import mediaIcon from "./assets/service-icons/media.png";
import meetIcon from "./assets/service-icons/meet.png";
import peopleIcon from "./assets/service-icons/people.png";
import securityIdentityIcon from "./assets/service-icons/security-identity.png";
import sheetsIcon from "./assets/service-icons/sheets.png";
import slidesIcon from "./assets/service-icons/slides.png";
import tasksIcon from "./assets/service-icons/tasks.png";
import webMobileIcon from "./assets/service-icons/web-mobile.png";
import {
  type BusinessConnector,
  type BusinessConnectorInfo,
  type BusinessSelection,
  ERROR_TEXT,
  HAS_BUSINESS_LEVEL,
  HAS_LEVEL,
  type RevokeOutcome,
  type SaveOutcome,
  type Selection,
  type Service,
  type ServiceInfo,
  type Status,
} from "./core";
import InstallWindow from "./windows/InstallWindow";
import SkillsWindow from "./windows/SkillsWindow";

/** The one action running right now; controls stay disabled while one runs. */
type Busy = "save" | "revoke" | "update" | "refresh" | null;

/** Feedback shown after an action. */
type Notice = { kind: "success" | "info" | "error"; text: string } | null;
type AccessChoice = "off" | "read" | "write";

const SERVICE_ICONS: Partial<Record<Service, string>> = {
  gmail: gmailIcon,
  calendar: calendarIcon,
  chat: chatIcon,
  meet: meetIcon,
  drive: driveIcon,
  docs: docsIcon,
  sheets: sheetsIcon,
  slides: slidesIcon,
  forms: formsIcon,
  tasks: tasksIcon,
  people: peopleIcon,
  "admin-reports": adminIcon,
  "admin-directory": adminIcon,
  "admin-groups": adminIcon,
  "cloud-identity": securityIdentityIcon,
};

const BUSINESS_ICONS: Record<BusinessConnector, string> = {
  youtube: mediaIcon,
  "google-ads": businessIntelligenceIcon,
  analytics: dataAnalyticsIcon,
  "search-console": webMobileIcon,
  "business-profile": mapsIcon,
  "merchant-center": marketplaceIcon,
};

function ServiceIcon({ src, label }: { src?: string; label: string }) {
  if (src !== undefined) {
    return (
      <img
        src={src}
        alt=""
        aria-hidden="true"
        className="h-7 w-7 shrink-0 object-contain"
      />
    );
  }
  return (
    <span
      aria-hidden="true"
      className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-slate-100 text-sm font-semibold text-slate-500 dark:bg-zinc-800 dark:text-zinc-300"
    >
      {label.charAt(0)}
    </span>
  );
}

function AvailabilityWarning({ message }: { message: string }) {
  return (
    <button
      type="button"
      title={message}
      aria-label={`Advertencia: ${message}`}
      className="ml-1 inline-flex h-6 w-6 items-center justify-center rounded-full text-base leading-none text-amber-600 hover:bg-amber-100 focus:outline-none focus:ring-2 focus:ring-amber-500 dark:text-amber-300 dark:hover:bg-amber-950"
      onClick={() => window.alert(message)}
    >
      ⚠
    </button>
  );
}

function serviceAccess(selection: Selection, service: Service): AccessChoice {
  if (HAS_LEVEL(selection, service, "write")) {
    return "write";
  }
  return HAS_LEVEL(selection, service, "read") ? "read" : "off";
}

function businessAccess(
  selection: BusinessSelection,
  connector: BusinessConnector,
): AccessChoice {
  if (HAS_BUSINESS_LEVEL(selection, connector, "write")) {
    return "write";
  }
  return HAS_BUSINESS_LEVEL(selection, connector, "read") ? "read" : "off";
}

function updateServiceAccess(
  selection: Selection,
  service: Service,
  access: AccessChoice,
): Selection {
  const withoutService = selection.filter((item) => item.service !== service);
  return access === "off"
    ? withoutService
    : [
        ...withoutService,
        { service, level: "read" },
        ...(access === "write" ? [{ service, level: "write" as const }] : []),
      ];
}

function updateBusinessAccess(
  selection: BusinessSelection,
  connector: BusinessConnector,
  access: AccessChoice,
): BusinessSelection {
  const withoutConnector = selection.filter(
    (item) => item.connector !== connector,
  );
  return access === "off"
    ? withoutConnector
    : [
        ...withoutConnector,
        { connector, level: "read" },
        ...(access === "write" ? [{ connector, level: "write" as const }] : []),
      ];
}

function AccessControl({
  value,
  onChange,
  disabled,
  writeDisabled = false,
  ariaLabel,
}: {
  value: AccessChoice;
  onChange: (value: AccessChoice) => void;
  disabled: boolean;
  writeDisabled?: boolean;
  ariaLabel: string;
}) {
  const options: Array<{
    value: AccessChoice;
    label: string;
    shortLabel: string;
  }> = [
    { value: "off", label: "Apagado", shortLabel: "Apagado" },
    { value: "read", label: "Solo Lectura", shortLabel: "Lectura" },
    { value: "write", label: "Acceso completo", shortLabel: "Completo" },
  ];

  return (
    <fieldset className="inline-flex rounded-lg border border-slate-200 p-0.5 dark:border-zinc-700">
      <legend className="sr-only">{ariaLabel}</legend>
      {options.map((option) => {
        const isSelected = value === option.value;
        const isDisabled =
          disabled || (option.value === "write" && writeDisabled);
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={isSelected}
            aria-label={option.label}
            disabled={isDisabled}
            onClick={() => onChange(option.value)}
            className={`rounded-md px-2 py-1 text-xs transition-colors sm:px-2.5 ${
              isSelected
                ? "bg-zinc-900 font-medium text-white shadow-sm dark:bg-zinc-100 dark:text-zinc-950"
                : "text-slate-600 hover:bg-slate-100 dark:text-zinc-300 dark:hover:bg-zinc-800"
            } disabled:cursor-not-allowed disabled:opacity-40`}
          >
            <span className="sm:hidden">{option.shortLabel}</span>
            <span className="hidden sm:inline">{option.label}</span>
          </button>
        );
      })}
    </fieldset>
  );
}

/**
 * The settings window of fraccional-googleworkspace-mcp: account, services, Save, Revoke all and the Advanced
 * settings. It also routes the skills and install windows to their own components.
 *
 * @example
 * <App />
 */
export default function App() {
  const windowName = new URLSearchParams(window.location.search).get("window");
  const [status, setStatus] = useState<Status | null>(null);
  const [profilePicture, setProfilePicture] = useState<string | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [catalogue, setCatalogue] = useState<ServiceInfo[]>([]);
  const [businessCatalogue, setBusinessCatalogue] = useState<
    BusinessConnectorInfo[]
  >([]);
  const [businessSelection, setBusinessSelection] = useState<BusinessSelection>(
    [],
  );
  const [catalogueError, setCatalogueError] = useState<string | null>(null);
  const [selection, setSelection] = useState<Selection>([]);
  const [busy, setBusy] = useState<Busy>(null);
  const [cancelling, setCancelling] = useState(false);
  const cancellingRef = useRef(false);
  const [authUrl, setAuthUrl] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice>(null);
  const [update, setUpdate] = useState<Update | null>(null);

  // biome-ignore lint/correctness/useExhaustiveDependencies: this startup effect intentionally runs once.
  useEffect(() => {
    if (windowName === "skills" || windowName === "install") {
      return;
    }
    const unlisten = listen<string>("auth-url", (event) =>
      setAuthUrl(event.payload),
    );
    loadAll();
    checkForUpdate();
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [windowName]);
  if (windowName === "skills") {
    return <SkillsWindow />;
  }

  if (windowName === "install") {
    return <InstallWindow />;
  }

  /**
   * Looks for a newer release on start. A failed check is silent (console only).
   *
   * @example
   * await checkForUpdate();
   */
  async function checkForUpdate() {
    try {
      setUpdate(await check());
    } catch (error) {
      console.error("Update check failed:", error);
    }
  }

  /**
   * Downloads and installs the available update, then restarts the app.
   *
   * @example
   * <button onClick={handleInstallUpdate}>Install and restart</button>
   */
  async function handleInstallUpdate() {
    if (update === null) {
      return;
    }
    setBusy("update");
    setNotice(null);
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (error) {
      setNotice({ kind: "error", text: ERROR_TEXT(error) });
    } finally {
      setBusy(null);
    }
  }

  /**
   * Loads the status, the service labels and the starting selection
   * (what Google granted when connected, the safe default otherwise).
   *
   * @example
   * await loadAll();
   */
  async function loadAll() {
    const [statusResult, catalogueResult, businessResult, defaultsResult] =
      await Promise.allSettled([
        invoke<Status>("status"),
        invoke<ServiceInfo[]>("catalogue"),
        invoke<BusinessConnectorInfo[]>("business_catalogue"),
        invoke<Selection>("default_selection"),
      ]);
    if (catalogueResult.status === "fulfilled") {
      setCatalogue(catalogueResult.value);
      setCatalogueError(null);
    } else {
      setCatalogueError(ERROR_TEXT(catalogueResult.reason));
    }
    if (businessResult.status === "fulfilled") {
      setBusinessCatalogue(businessResult.value);
    }
    if (statusResult.status === "fulfilled") {
      setStatus(statusResult.value);
      setStatusError(null);
      if (statusResult.value.account !== null) {
        try {
          setProfilePicture(await invoke<string | null>("profile_picture"));
        } catch (error) {
          console.warn("Could not load Google profile picture:", error);
          setProfilePicture(null);
        }
      } else {
        setProfilePicture(null);
      }
    } else {
      setStatusError(ERROR_TEXT(statusResult.reason));
    }
    if (
      statusResult.status === "fulfilled" &&
      statusResult.value.account !== null
    ) {
      setSelection(statusResult.value.granted);
      setBusinessSelection(statusResult.value.business_granted);
    } else if (defaultsResult.status === "fulfilled") {
      setSelection(defaultsResult.value);
    }
  }

  /**
   * Re-reads the token's effective permissions and resets the selection to them.
   *
   * @example
   * await handleRefresh();
   */
  async function handleRefresh() {
    setBusy("refresh");
    try {
      await loadAll();
    } finally {
      setBusy(null);
    }
  }

  /**
   * Reloads only the status (after Revoke all or an Advanced change).
   *
   * @example
   * await refreshStatus();
   */
  async function refreshStatus() {
    try {
      const nextStatus = await invoke<Status>("status");
      setStatus(nextStatus);
      setStatusError(null);
      setProfilePicture(
        nextStatus.account === null
          ? null
          : await invoke<string | null>("profile_picture"),
      );
    } catch (error) {
      setStatusError(ERROR_TEXT(error));
      setProfilePicture(null);
    }
  }

  /**
   * Saves the selection: runs the Google sign-in, asks before replacing foreign
   * gws credentials, then shows the result.
   *
   * @example
   * <button onClick={handleSave}>Save</button>
   */
  async function handleSave() {
    setBusy("save");
    setNotice(null);
    setAuthUrl(null);
    // An empty selection makes `save` revoke everything instead of signing in.
    const removedEverything =
      selection.length === 0 && businessSelection.length === 0;
    try {
      let outcome = await invoke<SaveOutcome>("save", {
        selection,
        businessSelection,
        confirmReplace: false,
      });
      if (outcome.kind === "needs_replace_confirmation") {
        const replace = await ask(
          "Es necesario actualizar los permisos. Si cancelas la operación, tus permisos actuales seguirán funcionando como antes.",
          {
            title: "Se volverá a abrir la pantalla de consentimiento",
            kind: "warning",
            okLabel: "Sí",
            cancelLabel: "No",
          },
        );
        if (!replace) {
          return;
        }
        outcome = await invoke<SaveOutcome>("save", {
          selection,
          businessSelection,
          confirmReplace: true,
        });
      }
      if (outcome.kind === "connected") {
        const account = outcome.status.account;
        setStatus(outcome.status);
        setStatusError(null);
        if (account === null) {
          setProfilePicture(null);
        } else {
          try {
            setProfilePicture(await invoke<string | null>("profile_picture"));
          } catch (error) {
            console.warn("Could not load Google profile picture:", error);
            setProfilePicture(null);
          }
        }
        setSelection(outcome.status.granted);
        setBusinessSelection(outcome.status.business_granted);
        if (account !== null) {
          setNotice({
            kind: "success",
            text: `Guardado. Conectado como ${account}.`,
          });
        } else if (removedEverything) {
          setNotice({
            kind: "success",
            text: "Guardado. Se eliminó todo el acceso.",
          });
        } else {
          // The login finished but status found no account: never report it as a removal.
          setNotice({
            kind: "error",
            text: `El inicio de sesión de Google terminó, pero no encuentro la cuenta en ${outcome.status.config_dir}. Vuelve a intentarlo.`,
          });
        }
      }
    } catch (error) {
      const text = ERROR_TEXT(error);
      if (text === "Google sign-in was cancelled") {
        setNotice({
          kind: "info",
          text: "Se canceló el inicio de sesión de Google. No se realizaron cambios.",
        });
      } else {
        setNotice({ kind: "error", text });
      }
      // A failed save may already have removed access (e.g. a reduced selection): show the truth.
      await refreshStatus();
    } finally {
      setBusy(null);
      cancellingRef.current = false;
      setCancelling(false);
      setAuthUrl(null);
    }
  }

  /**
   * Stops the Google sign-in that Save is waiting on.
   *
   * @example
   * <button onClick={handleCancel}>Cancel</button>
   */
  async function handleCancel() {
    if (cancellingRef.current) {
      return;
    }
    cancellingRef.current = true;
    setCancelling(true);
    try {
      await invoke("cancel_login");
    } catch (error) {
      setNotice({ kind: "error", text: ERROR_TEXT(error) });
      cancellingRef.current = false;
      setCancelling(false);
    }
  }

  /**
   * Revokes every access at Google and clears local credentials, after a confirm.
   *
   * @example
   * <button onClick={handleRevokeAll}>Revoke all</button>
   */
  async function handleRevokeAll() {
    const confirmed = await ask(
      "¿Revocar todo el acceso? Tus agentes perderán acceso a Google hasta que vuelvas a conectar la cuenta.",
      {
        title: "Revocar todo",
        kind: "warning",
        okLabel: "Revocar todo",
        cancelLabel: "Mantener acceso",
      },
    );
    if (!confirmed) {
      return;
    }
    setBusy("revoke");
    setNotice(null);
    try {
      let outcome = await invoke<RevokeOutcome>("revoke_all", {
        confirmForeign: false,
      });
      if (outcome.kind === "needs_foreign_confirmation") {
        const revokeForeign = await ask(
          "Estas credenciales de gws fueron creadas por otra aplicación. ¿Revocarlas de todas formas?",
          {
            title: "Revocar todo",
            kind: "warning",
            okLabel: "Sí",
            cancelLabel: "No",
          },
        );
        if (!revokeForeign) {
          setNotice({ kind: "info", text: "No se revocó nada." });
          return;
        }
        outcome = await invoke<RevokeOutcome>("revoke_all", {
          confirmForeign: true,
        });
      }
      if (outcome.kind === "revoked") {
        setNotice({ kind: "success", text: "Se revocó todo el acceso." });
      }
    } catch (error) {
      setNotice({ kind: "error", text: ERROR_TEXT(error) });
    } finally {
      await refreshStatus();
      setBusy(null);
    }
  }

  let accountText: string;
  if (status === null && statusError === null) {
    accountText = "Comprobando…";
  } else if (status === null) {
    accountText = "Estado no disponible";
  } else if (status.account !== null) {
    accountText = status.account;
  } else {
    accountText = "Sin conexión";
  }

  const accountInitials =
    status?.account
      ?.split("@")[0]
      .split(/[._-]/)
      .filter(Boolean)
      .slice(0, 2)
      .map((part) => part.charAt(0).toUpperCase())
      .join("") || "?";
  const disabled = busy !== null;
  const selectionKey = (
    items: Array<{
      level: "read" | "write";
      service?: string;
      connector?: string;
    }>,
  ) => {
    const effectivePermissions = new Set<string>();
    for (const item of items) {
      const resource = item.service ?? item.connector;
      if (resource === undefined) {
        continue;
      }
      effectivePermissions.add(`${resource}:read`);
      if (item.level === "write") {
        effectivePermissions.add(`${resource}:write`);
      }
    }
    return [...effectivePermissions].sort().join("|");
  };
  const hasChanges =
    status !== null &&
    (status.account === null
      ? selection.length > 0 || businessSelection.length > 0
      : selectionKey(selection) !== selectionKey(status.granted) ||
        selectionKey(businessSelection) !==
          selectionKey(status.business_granted));
  const saveDisabled = disabled || !hasChanges;
  const setAllAccess = (access: "read" | "write") => {
    let nextSelection = selection;
    for (const info of catalogue) {
      if (!info.enabled || (access === "write" && !info.write_enabled)) {
        continue;
      }
      nextSelection = updateServiceAccess(nextSelection, info.service, access);
    }

    let nextBusinessSelection = businessSelection;
    for (const info of businessCatalogue) {
      if (!info.enabled) {
        continue;
      }
      nextBusinessSelection = updateBusinessAccess(
        nextBusinessSelection,
        info.connector,
        access,
      );
    }

    setSelection(nextSelection);
    setBusinessSelection(nextBusinessSelection);
  };
  const statusDotClass =
    status === null || statusError !== null
      ? "bg-amber-400"
      : status.account !== null && status.token_valid
        ? "bg-emerald-500"
        : status.account !== null
          ? "bg-amber-400"
          : "bg-slate-400";

  let noticeClass =
    "border-red-200 bg-red-50 text-red-800 dark:border-red-900 dark:bg-red-950 dark:text-red-200";
  if (notice?.kind === "success") {
    noticeClass =
      "border-emerald-200 bg-emerald-50 text-emerald-800 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-200";
  } else if (notice?.kind === "info") {
    noticeClass =
      "border-slate-200 bg-slate-50 text-slate-700 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-200";
  }

  return (
    <>
      <main className="mx-auto min-h-screen max-w-xl space-y-6 bg-white p-8 pb-32 font-sans text-slate-900 dark:bg-zinc-950 dark:text-zinc-100">
        {update !== null && (
          <div className="flex items-center justify-between gap-3 rounded-lg border border-slate-200 bg-slate-50 p-3 text-sm dark:border-zinc-700 dark:bg-zinc-900">
            <span className="text-slate-700 dark:text-zinc-200">
              La versión {update.version} está disponible
            </span>
            <button
              type="button"
              className="rounded-md bg-slate-900 px-3 py-1.5 font-medium text-white hover:bg-slate-700 disabled:opacity-40 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-white"
              disabled={disabled}
              onClick={handleInstallUpdate}
            >
              {busy === "update" ? "Instalando…" : "Instalar y reiniciar"}
            </button>
          </div>
        )}

        <header>
          <div className="flex items-center justify-between gap-3">
            <div className="flex items-center gap-3">
              <img
                src={fraccionalIsotipo}
                alt="Fraccional"
                className="h-9 w-9 rounded-lg bg-zinc-900 p-1.5"
              />
              <h1 className="text-xl font-semibold">
                fraccional-googleworkspace-mcp
              </h1>
            </div>
            <button
              type="button"
              className="shrink-0 text-xs text-slate-500 underline-offset-4 hover:text-slate-900 hover:underline disabled:cursor-not-allowed disabled:opacity-40 dark:text-zinc-400 dark:hover:text-zinc-100"
              disabled={disabled}
              onClick={handleRefresh}
            >
              {busy === "refresh" ? "Refrescando…" : "Refrescar"}
            </button>
          </div>
          <p className="mt-1 text-sm text-slate-500 dark:text-zinc-400">
            Conecta tus agentes de IA a Google Workspace.
          </p>
        </header>

        <section className="space-y-2">
          {statusError !== null && (
            <p className="mt-1 text-sm text-slate-500 dark:text-zinc-400">
              {statusError}
            </p>
          )}
          {status !== null &&
            status.account !== null &&
            !status.token_valid && (
              <p className="mt-1 text-sm text-amber-700 dark:text-amber-300">
                La sesión expiró. Pulsa Guardar para iniciar sesión nuevamente.
              </p>
            )}
          {status !== null && !status.gws_available && (
            <p className="mt-1 text-sm text-slate-500 dark:text-zinc-400">
              gws se descargará al pulsar Guardar.
            </p>
          )}
        </section>

        <section className="space-y-3">
          <div className="flex flex-wrap items-baseline justify-between gap-3">
            <h2 className="text-sm font-semibold text-slate-700 dark:text-zinc-200">
              Lo que pueden usar tus agentes
            </h2>
            <div className="flex shrink-0 items-center gap-2 text-xs">
              <button
                type="button"
                className="text-slate-500 underline-offset-4 hover:text-slate-900 hover:underline disabled:cursor-not-allowed disabled:opacity-40 dark:text-zinc-400 dark:hover:text-zinc-100"
                disabled={disabled}
                onClick={() => setAllAccess("read")}
              >
                Todos lectura
              </button>
              <button
                type="button"
                className="text-slate-500 underline-offset-4 hover:text-slate-900 hover:underline disabled:cursor-not-allowed disabled:opacity-40 dark:text-zinc-400 dark:hover:text-zinc-100"
                disabled={disabled}
                onClick={() => setAllAccess("write")}
              >
                Todos completos
              </button>
            </div>
          </div>
          {catalogueError !== null && (
            <p className="text-sm text-slate-500 dark:text-zinc-400">
              Servicios no disponibles: {catalogueError}
            </p>
          )}
          <ul className="divide-y divide-slate-100 rounded-lg border border-slate-200 dark:divide-zinc-800 dark:border-zinc-700">
            {catalogue.map((info) => (
              <li
                key={info.service}
                className="flex items-center gap-3 px-3 py-2.5"
              >
                <ServiceIcon
                  src={SERVICE_ICONS[info.service]}
                  label={info.name}
                />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium">{info.name}</p>
                </div>
                <div className="flex shrink-0 items-center gap-1">
                  {info.availability_reason !== null && (
                    <AvailabilityWarning message={info.availability_reason} />
                  )}
                  <AccessControl
                    ariaLabel={`${info.name} nivel de acceso`}
                    value={serviceAccess(selection, info.service)}
                    onChange={(access) =>
                      setSelection(
                        updateServiceAccess(selection, info.service, access),
                      )
                    }
                    disabled={disabled || !info.enabled}
                    writeDisabled={!info.write_enabled}
                  />
                </div>
              </li>
            ))}
          </ul>

          <h2 className="pt-3 text-sm font-semibold text-slate-700 dark:text-zinc-200">
            Conectores empresariales de Google
          </h2>
          <ul className="divide-y divide-slate-100 rounded-lg border border-slate-200 dark:divide-zinc-800 dark:border-zinc-700">
            {businessCatalogue.map((info) => (
              <li
                key={info.connector}
                className="flex items-center gap-3 px-3 py-2.5"
              >
                <ServiceIcon
                  src={BUSINESS_ICONS[info.connector]}
                  label={info.name}
                />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-medium">{info.name}</p>
                </div>
                <div className="flex shrink-0 items-center gap-1">
                  {info.availability_reason !== null && (
                    <AvailabilityWarning message={info.availability_reason} />
                  )}
                  <AccessControl
                    ariaLabel={`${info.name} nivel de acceso`}
                    value={businessAccess(businessSelection, info.connector)}
                    onChange={(access) =>
                      setBusinessSelection(
                        updateBusinessAccess(
                          businessSelection,
                          info.connector,
                          access,
                        ),
                      )
                    }
                    disabled={disabled || !info.enabled}
                  />
                </div>
              </li>
            ))}
          </ul>

          {busy === "save" && (
            <div className="flex items-center justify-between gap-3 rounded-lg bg-slate-50 p-3 text-sm dark:bg-zinc-900">
              {authUrl === null && (
                <span className="text-slate-600 dark:text-zinc-300">
                  Preparando…
                </span>
              )}
              {authUrl !== null && (
                <span className="text-slate-600 dark:text-zinc-300">
                  Esperando el inicio de sesión de Google…{" "}
                  <button
                    type="button"
                    className="underline"
                    onClick={() => openUrl(authUrl)}
                  >
                    Abrir nuevamente
                  </button>
                </span>
              )}
            </div>
          )}
        </section>

        {notice !== null && (
          <p className={`rounded-lg border p-3 text-sm ${noticeClass}`}>
            {notice.text}
          </p>
        )}

        <section className="border-t border-slate-200 pt-6 text-sm dark:border-zinc-800">
          <button
            type="button"
            className="font-medium text-red-600 underline-offset-4 hover:underline disabled:cursor-not-allowed disabled:opacity-40 dark:text-red-400"
            disabled={disabled}
            onClick={handleRevokeAll}
          >
            Revocar todo
          </button>
        </section>
      </main>
      <div className="fixed bottom-3 left-1/2 z-30 w-[calc(100%-1.5rem)] max-w-xl -translate-x-1/2 rounded-2xl border border-slate-200/80 bg-white/95 px-4 py-3 shadow-[0_8px_30px_rgba(15,23,42,0.14)] backdrop-blur dark:border-zinc-700/80 dark:bg-zinc-950/95 dark:shadow-[0_8px_30px_rgba(0,0,0,0.35)]">
        <div className="flex items-center gap-3">
          <div className="relative shrink-0">
            {profilePicture !== null ? (
              <img
                src={profilePicture}
                alt=""
                className="h-10 w-10 rounded-full object-cover"
                referrerPolicy="no-referrer"
                onError={() => setProfilePicture(null)}
              />
            ) : (
              <div className="flex h-10 w-10 items-center justify-center rounded-full bg-slate-200 text-sm font-semibold text-slate-700 dark:bg-zinc-700 dark:text-zinc-100">
                {accountInitials}
              </div>
            )}
            <span
              className={`absolute -bottom-0.5 -right-0.5 h-3 w-3 rounded-full border-2 border-white dark:border-zinc-950 ${statusDotClass}`}
            />
          </div>
          <div className="min-w-0 flex-1">
            <p className="truncate text-sm font-medium">{accountText}</p>
            <p className="text-xs text-slate-500 dark:text-zinc-400">
              {status?.account === null && hasChanges
                ? "Esperando tu permiso"
                : hasChanges
                  ? "Hay cambios sin guardar"
                  : status?.account
                    ? "Todo guardado"
                    : "Selecciona servicios para conectar"}
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            {busy === "save" && (
              <button
                type="button"
                className="rounded-md border border-slate-300 px-3 py-2 text-sm hover:bg-slate-50 dark:border-zinc-600 dark:hover:bg-zinc-800"
                disabled={cancelling}
                aria-busy={cancelling}
                onClick={handleCancel}
              >
                {cancelling ? "Cancelando…" : "Cancelar"}
              </button>
            )}
            <button
              type="button"
              className="rounded-md bg-slate-900 px-4 py-2 text-sm font-medium text-white hover:bg-slate-700 disabled:cursor-not-allowed disabled:opacity-40 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-white"
              disabled={saveDisabled}
              onClick={handleSave}
            >
              {busy === "save"
                ? status?.account
                  ? "Guardando…"
                  : "Conectando…"
                : status?.account
                  ? "Guardar"
                  : "Conectar"}
            </button>
          </div>
        </div>
      </div>
    </>
  );
}
