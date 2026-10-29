// Page Compte (D-37) : connexion, création de compte, mot de passe oublié,
// déconnexion. Un compte n'est jamais obligatoire ; hors ligne, la session
// reste ouverte et l'état est simplement signalé.

import type { ParseKeys } from "i18next";
import { type FormEvent, useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  getAccountStatus,
  onAccountChanged,
  requestPasswordReset,
  signIn,
  signOut,
  signUp,
  toCommandError,
} from "../api";
import type { AccountStatus } from "../bindings/AccountStatus";
import { Button } from "../components/Button";
import { Spinner } from "../components/Spinner";
import { TextField } from "../components/TextField";

type Mode = "signIn" | "signUp" | "reset";

const MIN_PASSWORD = 8;

function looksLikeEmail(value: string): boolean {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value.trim());
}

export function AccountPanel() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<AccountStatus | null>(null);

  const reload = useCallback(() => {
    getAccountStatus()
      .then(setStatus)
      .catch(() => setStatus({ state: "unavailable" }));
  }, []);

  useEffect(() => {
    reload();
    const unlisten = onAccountChanged(reload).catch(() => undefined);
    return () => {
      void unlisten.then((stop) => stop?.()).catch(() => {});
    };
  }, [reload]);

  return (
    <section className="account" aria-labelledby="account-title">
      <h2 id="account-title" className="account__title">
        {t("settings.account.title")}
      </h2>
      <p className="section__help">{t("settings.account.intro")}</p>
      {status === null && <Spinner />}
      {status?.state === "unavailable" && <p>{t("settings.account.unavailable")}</p>}
      {status?.state === "signed_out" && <SignedOut onSignedIn={setStatus} />}
      {(status?.state === "signed_in" || status?.state === "offline") && (
        <SignedIn status={status} onSignedOut={setStatus} />
      )}
    </section>
  );
}

function SignedIn({
  status,
  onSignedOut,
}: {
  status: Extract<AccountStatus, { state: "signed_in" | "offline" }>;
  onSignedOut: (status: AccountStatus) => void;
}) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);

  return (
    <div className="account__panel">
      {status.state === "offline" ? (
        <p className="account__offline" role="status">
          {t("settings.account.offline", { email: status.email })}
        </p>
      ) : (
        <p role="status">{t("settings.account.signedIn", { email: status.email })}</p>
      )}
      <div>
        <Button
          disabled={busy}
          onClick={() => {
            setBusy(true);
            signOut()
              .then(onSignedOut)
              .catch(() => onSignedOut({ state: "signed_out" }))
              .finally(() => setBusy(false));
          }}
        >
          {t("settings.account.signOut")}
        </Button>
      </div>
    </div>
  );
}

function SignedOut({ onSignedIn }: { onSignedIn: (status: AccountStatus) => void }) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<Mode>("signIn");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const switchTo = (next: Mode) => {
    setMode(next);
    setError(null);
    setNotice(null);
    setPassword("");
    setConfirm("");
  };

  /** Vérifications locales, avant d'appeler le serveur. */
  const localError = (): string | null => {
    if (!looksLikeEmail(email)) {
      return t("settings.account.emailInvalid");
    }
    if (mode === "signUp") {
      if (password.length < MIN_PASSWORD) {
        return t("settings.account.passwordTooShort");
      }
      if (password !== confirm) {
        return t("settings.account.passwordMismatch");
      }
    }
    return null;
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const problem = localError();
    setError(problem);
    setNotice(null);
    if (problem) {
      return;
    }
    setBusy(true);
    try {
      if (mode === "signIn") {
        onSignedIn(await signIn(email, password));
      } else if (mode === "signUp") {
        const result = await signUp(email, password);
        if (result === "signed_in") {
          onSignedIn(await getAccountStatus());
        } else {
          setNotice(t("settings.account.confirmationSent", { email: email.trim() }));
          setMode("signIn");
          setPassword("");
          setConfirm("");
        }
      } else {
        await requestPasswordReset(email);
        setNotice(t("settings.account.resetSent"));
      }
    } catch (reason) {
      setError(t(toCommandError(reason).message_key as ParseKeys));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="account__panel">
      {mode !== "reset" && (
        <fieldset className="account__modes">
          <legend className="visually-hidden">{t("settings.account.modes")}</legend>
          {(["signIn", "signUp"] as const).map((item) => (
            <Button
              key={item}
              variant={mode === item ? "primary" : "secondary"}
              aria-pressed={mode === item}
              onClick={() => switchTo(item)}
            >
              {t(`settings.account.${item}`)}
            </Button>
          ))}
        </fieldset>
      )}
      {mode === "reset" && (
        <>
          <h3 className="account__subtitle">{t("settings.account.resetTitle")}</h3>
          <p className="section__help">{t("settings.account.resetIntro")}</p>
        </>
      )}
      <form className="account__form" onSubmit={(event) => void submit(event)} noValidate>
        <TextField
          label={t("settings.account.email")}
          type="email"
          autoComplete="email"
          value={email}
          onChange={(event) => setEmail(event.target.value)}
          required
        />
        {mode !== "reset" && (
          <TextField
            label={t("settings.account.password")}
            type="password"
            autoComplete={mode === "signUp" ? "new-password" : "current-password"}
            hint={mode === "signUp" ? t("settings.account.passwordHint") : undefined}
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            required
          />
        )}
        {mode === "signUp" && (
          <TextField
            label={t("settings.account.passwordConfirm")}
            type="password"
            autoComplete="new-password"
            value={confirm}
            onChange={(event) => setConfirm(event.target.value)}
            required
          />
        )}
        {error && (
          <p className="field__error" role="alert">
            {error}
          </p>
        )}
        {notice && (
          <p className="account__notice" role="status">
            {notice}
          </p>
        )}
        <div className="account__actions">
          <Button type="submit" variant="primary" disabled={busy}>
            {mode === "reset" ? t("settings.account.resetSend") : t(`settings.account.${mode}`)}
          </Button>
          {mode === "signIn" && (
            <Button onClick={() => switchTo("reset")}>{t("settings.account.forgot")}</Button>
          )}
          {mode === "reset" && (
            <Button onClick={() => switchTo("signIn")}>{t("settings.account.back")}</Button>
          )}
        </div>
      </form>
    </div>
  );
}
