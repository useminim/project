// Seul point d'appel des commandes Tauri : les composants n'appellent jamais
// `invoke` directement. Tout rejet devient une `ApiError` qui porte un
// `CommandError` typé ; un rejet inattendu (commande inconnue, panique côté
// Rust) devient une erreur `internal`.

import { type InvokeArgs, invoke } from "@tauri-apps/api/core";
import type { CommandError } from "../bindings/CommandError";
import type { ErrorCode } from "../bindings/ErrorCode";

// `satisfies` oblige à lister chaque code : un code ajouté côté Rust fait
// échouer `tsc` tant qu'il n'est pas reconnu ici.
const errorCodes = {
  validation_failed: true,
  not_found: true,
  conflict: true,
  storage_unavailable: true,
  io: true,
  internal: true,
} satisfies Record<ErrorCode, true>;

const internalError: CommandError = { code: "internal", message_key: "errors.internal" };

/** Erreur levée par `invokeCommand` quand une commande échoue. */
export class ApiError extends Error {
  readonly error: CommandError;

  constructor(error: CommandError) {
    super(error.message_key);
    this.name = "ApiError";
    this.error = error;
  }
}

function isErrorCode(value: unknown): value is ErrorCode {
  return typeof value === "string" && Object.hasOwn(errorCodes, value);
}

/** Reconnaît un `CommandError` sérialisé par Rust. */
export function isCommandError(value: unknown): value is CommandError {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const candidate = value as Record<string, unknown>;
  return isErrorCode(candidate.code) && typeof candidate.message_key === "string";
}

/**
 * Convertit le motif d'un rejet en `CommandError` : une `ApiError` levée par
 * `invokeCommand`, ou un `CommandError` brut.
 */
export function toCommandError(reason: unknown): CommandError {
  if (reason instanceof ApiError) {
    return reason.error;
  }
  return isCommandError(reason) ? reason : internalError;
}

/**
 * Appelle une commande Tauri.
 *
 * @throws {ApiError} si la commande échoue.
 */
export async function invokeCommand<T>(command: string, args?: InvokeArgs): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (reason) {
    throw new ApiError(toCommandError(reason));
  }
}
