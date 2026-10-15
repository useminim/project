import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { useUiStore } from "../state/ui";
import { Button } from "./Button";
import { ConfirmDialog } from "./ConfirmDialog";
import { Spinner } from "./Spinner";
import { TextField } from "./TextField";
import { Toaster } from "./Toaster";

function DeleteButton({ onConfirm }: { onConfirm: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button onClick={() => setOpen(true)}>Supprimer</Button>
      <ConfirmDialog
        open={open}
        onOpenChange={setOpen}
        title="Supprimer le workflow ?"
        description="Cette action est définitive."
        destructive
        onConfirm={onConfirm}
      />
    </>
  );
}

describe("ConfirmDialog", () => {
  it("confirms with the confirm button", async () => {
    const onConfirm = vi.fn();
    const user = userEvent.setup();
    render(<DeleteButton onConfirm={onConfirm} />);

    await user.click(screen.getByRole("button", { name: "Supprimer" }));
    const dialog = screen.getByRole("alertdialog", { name: "Supprimer le workflow ?" });
    expect(dialog.textContent).toContain("Cette action est définitive.");

    await user.click(screen.getByRole("button", { name: "Confirmer" }));

    expect(onConfirm).toHaveBeenCalledOnce();
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("cancels with the Escape key", async () => {
    const onConfirm = vi.fn();
    const user = userEvent.setup();
    render(<DeleteButton onConfirm={onConfirm} />);

    await user.click(screen.getByRole("button", { name: "Supprimer" }));
    await user.keyboard("{Escape}");

    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(onConfirm).not.toHaveBeenCalled();
  });
});

describe("Toaster", () => {
  it("shows a translated notification and closes it", async () => {
    const user = userEvent.setup();
    render(<Toaster />);

    act(() => {
      useUiStore.getState().showToast("error", "errors.not_found");
    });

    expect(screen.getByText("L'élément demandé est introuvable.")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Fermer" }));

    expect(useUiStore.getState().toasts).toEqual([]);
  });
});

describe("TextField", () => {
  it("links the label, the hint and the error to the input", () => {
    render(<TextField label="Nom" hint="100 caractères au plus" error="Le nom est vide" />);

    const input = screen.getByRole("textbox", { name: "Nom" });
    expect(input.getAttribute("aria-invalid")).toBe("true");
    const describedBy = input.getAttribute("aria-describedby") ?? "";
    const descriptions = describedBy
      .split(" ")
      .map((id) => document.getElementById(id)?.textContent);
    expect(descriptions).toEqual(["100 caractères au plus", "Le nom est vide"]);
  });
});

describe("Spinner", () => {
  it("announces the loading state", () => {
    render(<Spinner />);

    expect(screen.getByRole("status").textContent).toBe("Chargement…");
  });
});
