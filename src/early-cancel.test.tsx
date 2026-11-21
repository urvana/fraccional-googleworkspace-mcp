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
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import App from "./App";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  check: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: vi.fn() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn() }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: mocks.check }));

beforeEach(() => {
  vi.resetAllMocks();
  mocks.check.mockResolvedValue(null);
  mocks.invoke.mockImplementation((command: string) => {
    if (command === "status")
      return Promise.resolve({
        gws_available: true,
        account: "ana@example.com",
        granted: [{ service: "gmail", level: "read" }],
        business_granted: [],
        token_valid: true,
        config_dir: "/test",
        config_dir_is_default: true,
        key_storage: "auto",
        resolved_key_storage: "file",
        foreign_credentials: false,
      });
    if (command === "catalogue")
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
    if (["business_catalogue", "default_selection"].includes(command))
      return Promise.resolve([]);
    if (command === "profile_picture") return Promise.resolve(null);
    if (command === "cancel_login") return Promise.resolve();
    return Promise.reject(`unexpected ${command}`);
  });
});
afterEach(cleanup);

async function openWithChanges() {
  render(<App />);
  const gmail = await screen.findByRole("group", {
    name: "Gmail nivel de acceso",
  });
  fireEvent.click(
    within(gmail).getByRole("button", { name: "Acceso completo" }),
  );
}

it("enables cancellation only for the current admitted save and removes listeners", async () => {
  const listeners: Array<(event: { payload: string }) => void> = [];
  const stops: Array<ReturnType<typeof vi.fn>> = [];
  const rejectSaves: Array<(error: string) => void> = [];
  mocks.listen.mockImplementation(
    (event: string, handler: (event: { payload: string }) => void) => {
      const stop = vi.fn();
      if (event === "save-ready") {
        listeners.push(handler);
        stops.push(stop);
      }
      return Promise.resolve(stop);
    },
  );
  const baseInvoke = mocks.invoke.getMockImplementation();
  if (!baseInvoke) throw new Error("Missing invoke mock");
  mocks.invoke.mockImplementation((command: string, args?: unknown) =>
    command === "save"
      ? new Promise((_, reject) => rejectSaves.push(reject))
      : baseInvoke(command, args),
  );
  await openWithChanges();
  fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
  await waitFor(() => expect(rejectSaves).toHaveLength(1));
  const firstId = mocks.invoke.mock.calls.find(
    ([command]) => command === "save",
  )?.[1].saveId;
  expect(typeof firstId).toBe("string");
  const cancel = screen.getByRole("button", { name: "Cancelar" });
  expect(cancel.hasAttribute("disabled")).toBe(true);
  fireEvent.click(cancel);
  expect(mocks.invoke).not.toHaveBeenCalledWith(
    "cancel_login",
    expect.anything(),
  );
  act(() => listeners[0]({ payload: "old" }));
  expect(cancel.hasAttribute("disabled")).toBe(true);
  act(() => listeners[0]({ payload: firstId }));
  expect(cancel.hasAttribute("disabled")).toBe(false);
  fireEvent.click(cancel);
  expect(mocks.invoke).toHaveBeenCalledWith("cancel_login", {
    saveId: firstId,
  });
  act(() => listeners[0]({ payload: firstId }));
  expect(
    screen
      .getByRole("button", { name: "Cancelando…" })
      .hasAttribute("disabled"),
  ).toBe(true);
  await act(async () => rejectSaves[0]("Google sign-in was cancelled"));
  await screen.findByText(
    "Se canceló el inicio de sesión de Google. No se realizaron cambios.",
  );
  expect(stops[0]).toHaveBeenCalledTimes(1);

  fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
  await waitFor(() => expect(rejectSaves).toHaveLength(2));
  const calls = mocks.invoke.mock.calls.filter(
    ([command]) => command === "save",
  );
  const secondId = calls[1][1].saveId;
  expect(secondId).not.toBe(firstId);
  act(() => {
    listeners[0]({ payload: firstId });
    listeners[1]({ payload: firstId });
  });
  expect(
    screen.getByRole("button", { name: "Cancelar" }).hasAttribute("disabled"),
  ).toBe(true);
  act(() => listeners[1]({ payload: secondId }));
  expect(
    screen.getByRole("button", { name: "Cancelar" }).hasAttribute("disabled"),
  ).toBe(false);
  await act(async () => rejectSaves[1]("Google sign-in was cancelled"));
  expect(stops[1]).toHaveBeenCalledTimes(1);
});

it("does not start save when the readiness listener cannot be registered", async () => {
  mocks.listen.mockImplementation((event: string) =>
    event === "save-ready"
      ? Promise.reject("sin permisos")
      : Promise.resolve(vi.fn()),
  );
  await openWithChanges();
  fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
  await screen.findByText("No se pudo preparar la cancelación: sin permisos");
  expect(
    mocks.invoke.mock.calls.filter(([command]) => command === "save"),
  ).toHaveLength(0);
});

it("removes the readiness listener when the window unmounts", async () => {
  const stop = vi.fn();
  let rejectSave: (error: string) => void = () => {};
  mocks.listen.mockImplementation((event: string) =>
    Promise.resolve(event === "save-ready" ? stop : vi.fn()),
  );
  const baseInvoke = mocks.invoke.getMockImplementation();
  if (!baseInvoke) throw new Error("Missing invoke mock");
  mocks.invoke.mockImplementation((command: string, args?: unknown) =>
    command === "save"
      ? new Promise((_, reject) => {
          rejectSave = reject;
        })
      : baseInvoke(command, args),
  );
  const view = render(<App />);
  const gmail = await screen.findByRole("group", {
    name: "Gmail nivel de acceso",
  });
  fireEvent.click(
    within(gmail).getByRole("button", { name: "Acceso completo" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
  await waitFor(() =>
    expect(mocks.invoke).toHaveBeenCalledWith("save", expect.anything()),
  );
  view.unmount();
  expect(stop).toHaveBeenCalledTimes(1);
  await act(async () => rejectSave("Google sign-in was cancelled"));
  expect(stop).toHaveBeenCalledTimes(1);
});

it("does not start save if listener registration finishes after unmount", async () => {
  const stop = vi.fn();
  let finish: (unlisten: () => void) => void = () => {};
  mocks.listen.mockImplementation((event: string) =>
    event === "save-ready"
      ? new Promise((resolve) => {
          finish = resolve;
        })
      : Promise.resolve(vi.fn()),
  );
  const view = render(<App />);
  const gmail = await screen.findByRole("group", {
    name: "Gmail nivel de acceso",
  });
  fireEvent.click(
    within(gmail).getByRole("button", { name: "Acceso completo" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
  view.unmount();
  await act(async () => finish(stop));
  expect(stop).toHaveBeenCalledTimes(1);
  expect(
    mocks.invoke.mock.calls.filter(([command]) => command === "save"),
  ).toHaveLength(0);
});
