// Compte de l'utilisateur : une fonction typée par commande Tauri
// (`src-tauri/src/commands/account.rs`). Les jetons restent côté Rust.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AccountStatus } from "../bindings/AccountStatus";
import type { SignUpResult } from "../bindings/SignUpResult";
import { invokeCommand } from "./invoke";

/** Nom de l'événement émis quand l'état du compte change. */
export const ACCOUNT_CHANGED = "account_changed";

/** État du compte (rafraîchit la session si besoin). */
export function getAccountStatus(): Promise<AccountStatus> {
  return invokeCommand<AccountStatus>("account_status");
}

/** Ouvre une session. */
export function signIn(email: string, password: string): Promise<AccountStatus> {
  return invokeCommand<AccountStatus>("sign_in", { email, password });
}

/** Crée un compte. */
export function signUp(email: string, password: string): Promise<SignUpResult> {
  return invokeCommand<SignUpResult>("sign_up", { email, password });
}

/** Ferme la session et efface les jetons. */
export function signOut(): Promise<AccountStatus> {
  return invokeCommand<AccountStatus>("sign_out");
}

/** Envoie un e-mail de réinitialisation du mot de passe. */
export function requestPasswordReset(email: string): Promise<void> {
  return invokeCommand<void>("request_password_reset", { email });
}

/** Appelle `onChange` quand l'état du compte change ; renvoie la fonction de désabonnement. */
export function onAccountChanged(onChange: () => void): Promise<UnlistenFn> {
  return listen(ACCOUNT_CHANGED, () => onChange());
}
