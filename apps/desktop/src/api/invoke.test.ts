import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import type { CommandError } from "../bindings/CommandError";
import { ApiError, invokeCommand } from "./invoke";

afterEach(() => {
  clearMocks();
});

async function rejectionOf(promise: Promise<unknown>): Promise<unknown> {
  try {
    await promise;
  } catch (reason) {
    return reason;
  }
  throw new Error("la promesse aurait dû être rejetée");
}

describe("invokeCommand", () => {
  it("returns the command result", async () => {
    mockIPC((command, payload) => (command === "echo" ? payload : undefined));

    await expect(invokeCommand("echo", { value: 42 })).resolves.toEqual({ value: 42 });
  });

  it("converts a rejection into a typed CommandError", async () => {
    const commandError: CommandError = {
      code: "validation_failed",
      message_key: "errors.validation_failed",
      details: { kind: "validation", issues: [{ path: "name", code: "empty_name" }] },
    };
    mockIPC(() => {
      throw commandError;
    });

    const reason = await rejectionOf(invokeCommand("save_workflow"));

    expect(reason).toBeInstanceOf(ApiError);
    expect((reason as ApiError).error).toEqual(commandError);
  });

  it("turns an unexpected rejection into an internal error", async () => {
    mockIPC(() => {
      throw "command save_workflow not found";
    });

    const reason = await rejectionOf(invokeCommand("save_workflow"));

    expect((reason as ApiError).error).toEqual({
      code: "internal",
      message_key: "errors.internal",
    });
  });

  it("rejects an unknown error code", async () => {
    mockIPC(() => {
      throw { code: "disk_on_fire", message_key: "errors.disk_on_fire" };
    });

    const reason = await rejectionOf(invokeCommand("save_workflow"));

    expect((reason as ApiError).error.code).toBe("internal");
  });
});
