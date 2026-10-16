import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { render, screen } from "@testing-library/react";
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

  it("stays silent when the status cannot be read", async () => {
    mockIPC(() => {
      throw "command storage_status not found";
    });
    const { container } = render(<StorageBanner />);

    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(container.textContent).toBe("");
  });
});
