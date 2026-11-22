/** @vitest-environment jsdom */
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type { Selection, Status } from "./core";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  ask: vi.fn(),
  listen: vi.fn(),
  check: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: mocks.ask }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn() }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: mocks.check }));

const read: Selection = [{ service: "gmail", level: "read" }];
const write: Selection = [...read, { service: "gmail", level: "write" }];
const connected: Status = {
  gws_available: true,
  account: "user@example.com",
  granted: read,
  business_granted: [],
  token_valid: true,
  config_dir: "/test/config",
  config_dir_is_default: true,
  key_storage: "auto",
  resolved_key_storage: "file",
  foreign_credentials: false,
};
const disconnected: Status = {
  ...connected,
  account: null,
  granted: [],
  token_valid: false,
};

let readyListener: ((event: { payload: string }) => void) | null = null;

function emitSaveReady(saveId: string) {
  if (readyListener === null) throw new Error("Missing save-ready listener");
  readyListener({ payload: saveId });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.resetAllMocks();
  readyListener = null;
  mocks.listen.mockImplementation(
    (event: string, handler: (event: { payload: string }) => void) => {
      if (event === "save-ready") readyListener = handler;
      return Promise.resolve(() => {
        if (readyListener === handler) readyListener = null;
      });
    },
  );
  mocks.check.mockResolvedValue(null);
  mocks.invoke.mockImplementation((command: string) => {
    switch (command) {
      case "status":
        return Promise.resolve(connected);
      case "catalogue":
        return Promise.resolve([
          {
            service: "gmail",
            name: "Gmail",
            read: "",
            write: "",
            category: "workspace",
            enabled: true,
            write_enabled: true,
            availability_reason: null,
          },
        ]);
      case "business_catalogue":
      case "default_selection":
        return Promise.resolve([]);
      case "profile_picture":
        return Promise.resolve(null);
      case "scope_count":
        return Promise.resolve({ requested: 1, max: 47 });
      default:
        return Promise.reject(new Error(`Unexpected command: ${command}`));
    }
  });
});

afterEach(cleanup);

describe("Google scope limit", () => {
  it("warns and blocks Save while the selection needs more scopes than one sign-in accepts", async () => {
    const defaults = mocks.invoke.getMockImplementation();
    let requested = 52;
    mocks.invoke.mockImplementation((command: string, args?: unknown) =>
      command === "scope_count"
        ? Promise.resolve({ requested, max: 47 })
        : defaults?.(command, args),
    );
    const gmail = await openWithChanges();

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("pide 52 permisos");
    expect(alert.textContent).toContain("hasta 47");
    expect(screen.getByText("52/47")).toBeTruthy();
    const save = screen.getByRole("button", { name: "Guardar" });
    expect((save as HTMLButtonElement).disabled).toBe(true);

    // Still a change against the granted Gmail read, now under the limit.
    requested = 40;
    fireEvent.click(within(gmail).getByRole("button", { name: "Apagado" }));

    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(screen.getByText("40/47")).toBeTruthy();
    expect((save as HTMLButtonElement).disabled).toBe(false);
  });
});

async function openWithChanges() {
  render(<App />);
  const gmail = await screen.findByRole("group", {
    name: "Gmail nivel de acceso",
  });
  fireEvent.click(
    within(gmail).getByRole("button", { name: "Acceso completo" }),
  );
  return gmail;
}

describe("access lifecycle", () => {
  it.each([true, false])(
    "asks before replacing credentials (confirmed: %s)",
    async (confirmed) => {
      mocks.ask.mockResolvedValue(confirmed);
      await openWithChanges();
      mocks.invoke.mockImplementation(
        (
          command: string,
          args?: { confirmReplace?: boolean; saveId: string },
        ) => {
          if (command === "save") {
            if (!args) throw new Error("Missing save arguments");
            emitSaveReady(args.saveId);
            return Promise.resolve(
              args?.confirmReplace
                ? {
                    kind: "connected",
                    status: { ...connected, granted: write },
                  }
                : { kind: "needs_replace_confirmation" },
            );
          }
          if (command === "profile_picture") return Promise.resolve(null);
          return Promise.reject(new Error(`Unexpected command: ${command}`));
        },
      );

      fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
      await waitFor(() => expect(mocks.ask).toHaveBeenCalledTimes(1));
      expect(mocks.ask).toHaveBeenCalledWith(
        "Es necesario actualizar los permisos. Si cancelas la operación, tus permisos actuales seguirán funcionando como antes.",
        expect.objectContaining({
          title: "Se volverá a abrir la pantalla de consentimiento",
        }),
      );
      if (confirmed) {
        expect(
          await screen.findByText("Guardado. Conectado como user@example.com."),
        ).toBeTruthy();
        expect(mocks.invoke).toHaveBeenCalledWith("save", {
          selection: write,
          businessSelection: [],
          confirmReplace: true,
          saveId: expect.any(String),
        });
        const saves = mocks.invoke.mock.calls.filter(
          ([command]) => command === "save",
        );
        expect(saves[1][1].saveId).not.toBe(saves[0][1].saveId);
      } else {
        await waitFor(() =>
          expect(
            screen
              .getByRole("button", { name: "Guardar" })
              .hasAttribute("disabled"),
          ).toBe(false),
        );
        expect(screen.getByText("Hay cambios sin guardar")).toBeTruthy();
        expect(mocks.invoke).not.toHaveBeenCalledWith(
          "save",
          expect.objectContaining({ confirmReplace: true }),
        );
      }
      expect(mocks.invoke).toHaveBeenCalledWith("save", {
        selection: write,
        businessSelection: [],
        confirmReplace: false,
        saveId: expect.any(String),
      });
    },
  );

  it("cancels a pending login once, keeps controls disabled, and refreshes the account", async () => {
    const save = deferred<unknown>();
    const cancel = deferred<unknown>();
    const gmail = await openWithChanges();
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "save") return save.promise;
      if (command === "cancel_login") return cancel.promise;
      if (command === "status") return Promise.resolve(connected);
      if (command === "profile_picture") return Promise.resolve(null);
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });

    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    expect(
      screen
        .getByRole("button", { name: "Guardando…" })
        .hasAttribute("disabled"),
    ).toBe(true);
    expect(
      within(gmail)
        .getByRole("button", { name: "Solo Lectura" })
        .hasAttribute("disabled"),
    ).toBe(true);
    await waitFor(() =>
      expect(mocks.invoke).toHaveBeenCalledWith(
        "save",
        expect.objectContaining({ saveId: expect.any(String) }),
      ),
    );
    const saveId = mocks.invoke.mock.calls.find(
      ([command]) => command === "save",
    )?.[1].saveId;
    expect(
      screen.getByRole("button", { name: "Cancelar" }).hasAttribute("disabled"),
    ).toBe(true);
    act(() => emitSaveReady("unrelated-save"));
    expect(
      screen.getByRole("button", { name: "Cancelar" }).hasAttribute("disabled"),
    ).toBe(true);
    act(() => emitSaveReady(saveId));
    expect(
      screen.getByRole("button", { name: "Cancelar" }).hasAttribute("disabled"),
    ).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    const cancelling = screen.getByRole("button", { name: "Cancelando…" });
    expect(cancelling.hasAttribute("disabled")).toBe(true);
    fireEvent.click(cancelling);
    expect(
      mocks.invoke.mock.calls.filter(([command]) => command === "cancel_login"),
    ).toHaveLength(1);
    expect(mocks.invoke).toHaveBeenCalledWith("cancel_login", { saveId });
    await act(async () => {
      cancel.resolve(undefined);
      save.reject("Google sign-in was cancelled");
    });
    expect(
      await screen.findByText(
        "Se canceló el inicio de sesión de Google. No se realizaron cambios.",
      ),
    ).toBeTruthy();
    expect(screen.getByText("user@example.com")).toBeTruthy();
    expect(screen.getByText("Hay cambios sin guardar")).toBeTruthy();
    expect(
      mocks.invoke.mock.calls.filter(([command]) => command === "status"),
    ).toHaveLength(2);
  });

  it("shows a cancellation error and allows retry", async () => {
    const save = deferred<unknown>();
    await openWithChanges();
    mocks.invoke.mockImplementation(
      (command: string, args?: { saveId: string }) => {
        if (command === "save") {
          if (!args) throw new Error("Missing save arguments");
          emitSaveReady(args.saveId);
          return save.promise;
        }
        if (command === "cancel_login")
          return Promise.reject("No se pudo cancelar");
        if (command === "status") return Promise.resolve(connected);
        if (command === "profile_picture") return Promise.resolve(null);
        return Promise.reject(new Error(`Unexpected command: ${command}`));
      },
    );
    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() =>
      expect(
        screen
          .getByRole("button", { name: "Cancelar" })
          .hasAttribute("disabled"),
      ).toBe(false),
    );
    const saveId = mocks.invoke.mock.calls.find(
      ([command]) => command === "save",
    )?.[1].saveId;
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(await screen.findByText("No se pudo cancelar")).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Cancelar" }).hasAttribute("disabled"),
    ).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
    await waitFor(() =>
      expect(
        mocks.invoke.mock.calls.filter(
          ([command]) => command === "cancel_login",
        ),
      ).toHaveLength(2),
    );
    expect(mocks.invoke).toHaveBeenCalledWith("cancel_login", { saveId });
    await act(async () => {
      save.reject("Google sign-in was cancelled");
    });
    await screen.findByText(
      "Se canceló el inicio de sesión de Google. No se realizaron cambios.",
    );
  });

  it("shows a failed save and the refreshed account after access was removed", async () => {
    await openWithChanges();
    mocks.invoke.mockImplementation(
      (command: string, args?: { saveId: string }) => {
        if (command === "save") {
          if (!args) throw new Error("Missing save arguments");
          emitSaveReady(args.saveId);
          return Promise.reject("Falló el guardado");
        }
        if (command === "status") return Promise.resolve(disconnected);
        return Promise.reject(new Error(`Unexpected command: ${command}`));
      },
    );
    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    expect(await screen.findByText("Falló el guardado")).toBeTruthy();
    await screen.findByText("Sin conexión");
    expect(screen.getByText("Esperando tu permiso")).toBeTruthy();
  });

  it("does not revoke when the initial confirmation is declined", async () => {
    render(<App />);
    await screen.findByText("user@example.com");
    mocks.ask.mockResolvedValue(false);
    fireEvent.click(screen.getByRole("button", { name: "Revocar todo" }));
    await waitFor(() => expect(mocks.ask).toHaveBeenCalledTimes(1));
    expect(
      mocks.invoke.mock.calls.filter(([command]) => command === "revoke_all"),
    ).toHaveLength(0);
    expect(screen.getByText("user@example.com")).toBeTruthy();
  });

  it.each([true, false])(
    "confirms foreign revocation separately (confirmed: %s)",
    async (confirmed) => {
      render(<App />);
      await screen.findByText("user@example.com");
      mocks.ask.mockResolvedValueOnce(true).mockResolvedValueOnce(confirmed);
      mocks.invoke.mockImplementation(
        (command: string, args?: { confirmForeign?: boolean }) => {
          if (command === "revoke_all")
            return Promise.resolve(
              args?.confirmForeign
                ? { kind: "revoked" }
                : { kind: "needs_foreign_confirmation" },
            );
          if (command === "status")
            return Promise.resolve(confirmed ? disconnected : connected);
          if (command === "profile_picture") return Promise.resolve(null);
          return Promise.reject(new Error(`Unexpected command: ${command}`));
        },
      );
      fireEvent.click(screen.getByRole("button", { name: "Revocar todo" }));
      expect(
        await screen.findByText(
          confirmed ? "Se revocó todo el acceso." : "No se revocó nada.",
        ),
      ).toBeTruthy();
      await screen.findByText(confirmed ? "Sin conexión" : "user@example.com");
      expect(mocks.ask).toHaveBeenCalledTimes(2);
      expect(mocks.invoke).toHaveBeenCalledWith("revoke_all", {
        confirmForeign: false,
      });
      expect(
        mocks.invoke.mock.calls.filter(([command]) => command === "revoke_all"),
      ).toHaveLength(confirmed ? 2 : 1);
      if (confirmed)
        expect(mocks.invoke).toHaveBeenCalledWith("revoke_all", {
          confirmForeign: true,
        });
      await waitFor(() =>
        expect(
          screen
            .getByRole("button", { name: "Revocar todo" })
            .hasAttribute("disabled"),
        ).toBe(false),
      );
    },
  );

  it("shows both revocation and status refresh errors", async () => {
    render(<App />);
    await screen.findByText("user@example.com");
    mocks.ask.mockResolvedValue(true);
    mocks.invoke.mockImplementation((command: string) => {
      if (command === "revoke_all")
        return Promise.reject("Falló la revocación");
      if (command === "status") return Promise.reject("Falló el estado");
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
    fireEvent.click(screen.getByRole("button", { name: "Revocar todo" }));
    expect(await screen.findByText("Falló la revocación")).toBeTruthy();
    expect(await screen.findByText("Falló el estado")).toBeTruthy();
    await waitFor(() =>
      expect(
        screen
          .getByRole("button", { name: "Revocar todo" })
          .hasAttribute("disabled"),
      ).toBe(false),
    );
  });
});
