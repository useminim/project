import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ErrorBoundary } from "./ErrorBoundary";

function Broken(): never {
  throw new Error("Dossier médical de Mme Martin");
}

describe("ErrorBoundary", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("shows a translated message instead of the broken view", async () => {
    // React signale l'erreur dans la console : on la masque pendant le test.
    vi.spyOn(console, "error").mockImplementation(() => {});
    const reload = vi.fn();
    const user = userEvent.setup();

    render(
      <ErrorBoundary onReload={reload}>
        <Broken />
      </ErrorBoundary>,
    );

    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("Une erreur inattendue est survenue");
    expect(alert.textContent).not.toContain("Martin");

    await user.click(screen.getByRole("button", { name: "Recharger" }));
    expect(reload).toHaveBeenCalledOnce();
  });

  it("renders its children when nothing fails", () => {
    render(
      <ErrorBoundary>
        <p>Contenu</p>
      </ErrorBoundary>,
    );

    expect(screen.getByText("Contenu")).toBeTruthy();
  });
});
