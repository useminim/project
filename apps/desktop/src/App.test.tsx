import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { App } from "./App";

describe("App", () => {
  it("opens on the workflows section", () => {
    render(<App />);

    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Workflows");
    expect(screen.getByRole("button", { name: "Workflows" }).getAttribute("aria-current")).toBe(
      "page",
    );
  });

  it("navigates between sections with the keyboard", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Workflows, Zone d'attente, Historique : troisième élément focalisable.
    await user.tab();
    await user.tab();
    await user.tab();
    const history = screen.getByRole("button", { name: "Historique" });
    expect(document.activeElement).toBe(history);

    await user.keyboard("{Enter}");

    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Historique");
    expect(history.getAttribute("aria-current")).toBe("page");
    expect(
      screen.getByRole("button", { name: "Workflows" }).getAttribute("aria-current"),
    ).toBeNull();

    await user.tab();
    await user.keyboard(" ");

    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Paramètres");
  });

  it("labels the main navigation", () => {
    render(<App />);

    expect(screen.getByRole("navigation", { name: "Navigation principale" })).toBeTruthy();
  });
});
