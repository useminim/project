import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import type { StorageStatus } from "../bindings/StorageStatus";
import { StorageBanner } from "./StorageBanner";

afterEach(() => {
  clearMocks();
});

function withStatus(status: StorageStatus) {
  mockIPC((command) => (command === "storage_status" ? status : undefined));
}

describe("StorageBanner", () => {
  it("shows nothing when storage is healthy", async () => {
    withStatus({ available: true, reduced_protection: false, recovered: null });
    const { container } = render(<StorageBanner />);

    // Laisse la promesse de la commande se résoudre.
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(container.textContent).toBe("");
  });

  it("reports unavailable storage as an alert", async () => {
    withStatus({ available: false, reduced_protection: false, recovered: null });
    render(<StorageBanner />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("le trousseau du système refuse l'accès");
  });

  it("reports reduced protection and a set-aside database", async () => {
    withStatus({ available: true, reduced_protection: true, recovered: "key_lost" });
    render(<StorageBanner />);

    const messages = await screen.findAllByRole("status");
    expect(messages.map((message) => message.textContent)).toEqual([
      expect.stringContaining("La clé de vos données locales a été perdue"),
      expect.stringContaining("Protection réduite"),
    ]);
  });

  it("hides the reduced protection warning for good once dismissed", async () => {
    withStatus({ available: true, reduced_protection: true, recovered: null });
    const user = userEvent.setup();
    const { unmount } = render(<StorageBanner />);

    await user.click(await screen.findByRole("button", { name: "Masquer ce message" }));

    expect(screen.queryByText(/Protection réduite/)).toBeNull();
    unmount();
    render(<StorageBanner />);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(screen.queryByText(/Protection réduite/)).toBeNull();
  });

  it("keeps other messages when the warning is dismissed", async () => {
    withStatus({ available: true, reduced_protection: true, recovered: "corrupted" });
    const user = userEvent.setup();
    render(<StorageBanner />);

    await user.click(await screen.findByRole("button", { name: "Masquer ce message" }));

    expect(screen.getByRole("status").textContent).toContain("endommagées");
  });

  it("cannot dismiss an unavailable storage error", async () => {
    withStatus({ available: false, reduced_protection: false, recovered: null });
    render(<StorageBanner />);

    await screen.findByRole("alert");
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("stays silent when the status cannot be read", async () => {
    mockIPC(() => {
      throw "command storage_status not found";
    });
    const { container } = render(<StorageBanner />);

    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(container.textContent).toBe("");
  });
});
