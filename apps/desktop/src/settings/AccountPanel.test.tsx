import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import type { AccountStatus } from "../bindings/AccountStatus";
import { AccountPanel } from "./AccountPanel";

afterEach(() => {
  cleanup();
  clearMocks();
});

const plain = (text: string | null | undefined) => (text ?? "").replaceAll(" ", " ");

/** Backend simulé : `handle` répond aux commandes de compte. */
function backend(
  status: AccountStatus,
  handle: (command: string, args: Record<string, unknown>) => unknown = () => undefined,
) {
  const calls: Array<{ command: string; args: Record<string, unknown> }> = [];
  mockIPC(
    (command, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      calls.push({ command, args });
      return command === "account_status" ? status : handle(command, args);
    },
    { shouldMockEvents: true },
  );
  return calls;
}

describe("AccountPanel", () => {
  it("explains that accounts are optional and unavailable in this version", async () => {
    backend({ state: "unavailable" });
    render(<AccountPanel />);

    expect(await screen.findByText(/pas encore disponibles/)).toBeTruthy();
    expect(screen.getByText(/n'est pas nécessaire pour utiliser minim/)).toBeTruthy();
    expect(screen.queryByLabelText("Mot de passe")).toBeNull();
  });

  it("signs in", async () => {
    const calls = backend({ state: "signed_out" }, (command) =>
      command === "sign_in" ? { state: "signed_in", email: "alice@example.com" } : undefined,
    );
    const user = userEvent.setup();
    render(<AccountPanel />);

    await user.type(await screen.findByLabelText("Adresse e-mail"), "alice@example.com");
    await user.type(screen.getByLabelText("Mot de passe"), "correct horse");
    await user.click(screen.getAllByRole("button", { name: "Se connecter" }).at(-1) as HTMLElement);

    expect(await screen.findByText("Connecté avec alice@example.com.")).toBeTruthy();
    expect(calls).toContainEqual({
      command: "sign_in",
      args: { email: "alice@example.com", password: "correct horse" },
    });
  });

  it("shows a translated error and keeps the form", async () => {
    backend({ state: "signed_out" }, () => {
      throw { code: "invalid_credentials", message_key: "errors.invalid_credentials" };
    });
    const user = userEvent.setup();
    render(<AccountPanel />);

    await user.type(await screen.findByLabelText("Adresse e-mail"), "alice@example.com");
    await user.type(screen.getByLabelText("Mot de passe"), "faux");
    await user.click(screen.getAllByRole("button", { name: "Se connecter" }).at(-1) as HTMLElement);

    expect((await screen.findByRole("alert")).textContent).toBe(
      "Adresse e-mail ou mot de passe incorrect.",
    );
    expect((screen.getByLabelText("Adresse e-mail") as HTMLInputElement).value).toBe(
      "alice@example.com",
    );
  });

  it("checks the sign-up form before calling the server", async () => {
    const calls = backend({ state: "signed_out" }, (command) =>
      command === "sign_up" ? "confirmation_sent" : undefined,
    );
    const user = userEvent.setup();
    render(<AccountPanel />);

    await user.click(await screen.findByRole("button", { name: "Créer un compte" }));
    await user.type(screen.getByLabelText("Adresse e-mail"), "pas-une-adresse");
    await user.click(
      screen.getAllByRole("button", { name: "Créer un compte" }).at(-1) as HTMLElement,
    );
    expect((await screen.findByRole("alert")).textContent).toBe(
      "Saisissez une adresse e-mail valide.",
    );

    await user.clear(screen.getByLabelText("Adresse e-mail"));
    await user.type(screen.getByLabelText("Adresse e-mail"), "bob@example.com");
    await user.type(screen.getByLabelText("Mot de passe"), "longmotdepasse");
    await user.type(screen.getByLabelText("Confirmer le mot de passe"), "autre");
    await user.click(
      screen.getAllByRole("button", { name: "Créer un compte" }).at(-1) as HTMLElement,
    );
    expect(screen.getByRole("alert").textContent).toBe(
      "Les deux mots de passe ne sont pas identiques.",
    );
    expect(calls.some((call) => call.command === "sign_up")).toBe(false);

    await user.clear(screen.getByLabelText("Confirmer le mot de passe"));
    await user.type(screen.getByLabelText("Confirmer le mot de passe"), "longmotdepasse");
    await user.click(
      screen.getAllByRole("button", { name: "Créer un compte" }).at(-1) as HTMLElement,
    );

    expect(plain((await screen.findByRole("status")).textContent)).toContain(
      "Ouvrez le lien envoyé à bob@example.com",
    );
    expect(calls).toContainEqual({
      command: "sign_up",
      args: { email: "bob@example.com", password: "longmotdepasse" },
    });
  });

  it("sends a password reset link", async () => {
    const calls = backend({ state: "signed_out" }, () => null);
    const user = userEvent.setup();
    render(<AccountPanel />);

    await user.click(await screen.findByRole("button", { name: /Mot de passe oublié/ }));
    await user.type(screen.getByLabelText("Adresse e-mail"), "alice@example.com");
    await user.click(screen.getByRole("button", { name: "Envoyer le lien" }));

    expect(await screen.findByText(/un e-mail vient d'être envoyé/)).toBeTruthy();
    expect(calls).toContainEqual({
      command: "request_password_reset",
      args: { email: "alice@example.com" },
    });
    await user.click(screen.getByRole("button", { name: "Retour à la connexion" }));
    expect(screen.getByLabelText("Mot de passe")).toBeTruthy();
  });

  it("shows the offline state without blocking and signs out", async () => {
    const calls = backend({ state: "offline", email: "alice@example.com" }, (command) =>
      command === "sign_out" ? { state: "signed_out" } : undefined,
    );
    const user = userEvent.setup();
    render(<AccountPanel />);

    expect(
      await screen.findByText(/Hors ligne\s:\svous restez connecté avec alice@example\.com/),
    ).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Se déconnecter" }));

    expect(await screen.findByLabelText("Mot de passe")).toBeTruthy();
    expect(calls.some((call) => call.command === "sign_out")).toBe(true);
  });
});
