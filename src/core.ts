/**
 * TypeScript mirror of the core contract (`crates/core/src/types.rs`) as the Tauri
 * commands serialise it. Errors arrive as plain strings (the `CoreError` display message).
 */

/** A Google Workspace service, e.g. `"gmail"`. */
export type Service =
  | "gmail"
  | "calendar"
  | "chat"
  | "meet"
  | "drive"
  | "docs"
  | "sheets"
  | "slides"
  | "forms"
  | "tasks"
  | "people"
  | "admin-reports"
  | "admin-directory"
  | "admin-groups"
  | "cloud-identity";

/** Access level for one service; `write` always implies `read`. */
export type Level = "read" | "write";

/** One checkbox of the UI, e.g. `{ service: "gmail", level: "read" }`. */
export type ServiceLevel = { service: Service; level: Level };

/** Granted or requested levels: a JSON array of `ServiceLevel`. */
export type Selection = ServiceLevel[];

/** Where gws keeps its encryption key. */
export type KeyStorage = "auto" | "keyring" | "file";

/** What the `status` command reports. */
export type Status = {
  gws_available: boolean;
  account: string | null;
  granted: Selection;
  business_granted: BusinessSelection;
  token_valid: boolean;
  config_dir: string;
  config_dir_is_default: boolean;
  key_storage: KeyStorage;
  resolved_key_storage: KeyStorage;
  foreign_credentials: boolean;
};

/** Result of the `save` command. */
export type SaveOutcome =
  | { kind: "connected"; status: Status }
  | { kind: "needs_replace_confirmation" };

/** Result of the `revoke_all` command. */
export type RevokeOutcome =
  | { kind: "revoked" }
  | { kind: "needs_foreign_confirmation" };

/** Plain-language labels for one service row. */
export type ServiceInfo = {
  service: Service;
  name: string;
  read: string;
  write: string;
  category: "workspace" | "admin";
  enabled: boolean;
  write_enabled: boolean;
  availability_reason: string | null;
};

/** Google APIs that use the shared OAuth account but are not gws services yet. */
export type BusinessConnector =
  | "youtube"
  | "google-ads"
  | "analytics"
  | "search-console"
  | "business-profile"
  | "merchant-center";

/** Planned business connector shown separately from the Workspace scope catalogue. */
export type BusinessConnectorInfo = {
  connector: BusinessConnector;
  name: string;
  read: string;
  write: string;
  enabled: boolean;
  availability_reason: string | null;
};

export type BusinessAccess = { connector: BusinessConnector; level: Level };
export type BusinessSelection = BusinessAccess[];

/**
 * What the `install_prompt` command returns: the text a person pastes into an agent so that it
 * connects itself to fraccional-googleworkspace-mcp, plus an optional warning to show above the steps.
 */
export type InstallPrompt = { text: string; warning: string | null };

/**
 * Whether a selection holds a service level.
 *
 * @example
 * HAS_LEVEL([{ service: "gmail", level: "read" }], "gmail", "read"); // true
 */
export function HAS_LEVEL(
  selection: Selection,
  service: Service,
  level: Level,
): boolean {
  return selection.some(
    (item) => item.service === service && item.level === level,
  );
}

export function HAS_BUSINESS_LEVEL(
  selection: BusinessSelection,
  connector: BusinessConnector,
  level: Level,
): boolean {
  return selection.some(
    (item) => item.connector === connector && item.level === level,
  );
}

export function TOGGLE_BUSINESS_LEVEL(
  selection: BusinessSelection,
  connector: BusinessConnector,
  level: Level,
  checked: boolean,
): BusinessSelection {
  const levels: Level[] =
    checked && level === "write" ? ["read", "write"] : [level];
  const others = selection.filter(
    (item) => item.connector !== connector || !levels.includes(item.level),
  );
  if (!checked) {
    return others;
  }
  return [...others, ...levels.map((added) => ({ connector, level: added }))];
}

/**
 * Ticks or unticks one checkbox and keeps the selection valid:
 * ticking Write also ticks Read, unticking Read also unticks Write.
 *
 * @example
 * TOGGLE_LEVEL([], "gmail", "write", true);
 * // [{ service: "gmail", level: "read" }, { service: "gmail", level: "write" }]
 */
export function TOGGLE_LEVEL(
  selection: Selection,
  service: Service,
  level: Level,
  checked: boolean,
): Selection {
  let levels: Level[];
  if (checked && level === "write") {
    levels = ["read", "write"];
  } else if (!checked && level === "read") {
    levels = ["read", "write"];
  } else {
    levels = [level];
  }
  const others = selection.filter(
    (item) => item.service !== service || !levels.includes(item.level),
  );
  if (!checked) {
    return others;
  }
  return [...others, ...levels.map((added) => ({ service, level: added }))];
}

/**
 * Turns whatever a rejected `invoke` returned into a message for the person.
 *
 * @example
 * ERROR_TEXT("gws failed (exit 2): No credentials"); // "gws failed (exit 2): No credentials"
 * ERROR_TEXT(new Error("boom")); // "boom"
 */
export function ERROR_TEXT(error: unknown): string {
  if (typeof error === "string") {
    return error;
  } else if (error instanceof Error) {
    return error.message;
  } else {
    return String(error);
  }
}
