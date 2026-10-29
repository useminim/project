// Écran « Workflows » : liste dans l'ordre d'évaluation, activation,
// suppression et réordonnancement (glisser-déposer à la souris ou au
// clavier, ou boutons « monter » / « descendre »). La liste est relue à
// chaque événement `workflows_changed`.

import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { ParseKeys } from "i18next";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  deleteWorkflow,
  listWorkflows,
  onWorkflowsChanged,
  reorderWorkflows,
  setWorkflowEnabled,
  toCommandError,
} from "../api";
import type { Workflow } from "../bindings/Workflow";
import { Button } from "../components/Button";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { EmptyState } from "../components/EmptyState";
import { Spinner } from "../components/Spinner";
import { Switch } from "../components/Switch";
import { useUiStore } from "../state/ui";
import { useWizardStore } from "../state/wizard";
import { describeActions, describeTrigger } from "./summary";
import { draftOf, newDraft } from "./wizard/draft";
import { WizardScreen } from "./wizard/WizardScreen";

/** Liste des workflows, ou le wizard si une création ou une modification est en cours. */
export function WorkflowsScreen() {
  const wizardOpen = useWizardStore((state) => state.session !== null);
  return wizardOpen ? <WizardScreen /> : <WorkflowList />;
}

function WorkflowList() {
  const { t } = useTranslation();
  const startWizard = useWizardStore((state) => state.start);
  const showToast = useUiStore((state) => state.showToast);
  const [workflows, setWorkflows] = useState<Workflow[] | null>(null);
  const [toDelete, setToDelete] = useState<Workflow | null>(null);

  const reload = useCallback(async () => {
    try {
      setWorkflows(await listWorkflows());
    } catch (reason) {
      showToast("error", toCommandError(reason).message_key as ParseKeys);
      setWorkflows((current) => current ?? []);
    }
  }, [showToast]);

  /** Affiche l'erreur ; un conflit recharge la liste. */
  const report = useCallback(
    (reason: unknown) => {
      const error = toCommandError(reason);
      if (error.code === "conflict") {
        showToast("error", "workflows.conflict");
      } else {
        showToast("error", error.message_key as ParseKeys);
      }
      void reload();
    },
    [reload, showToast],
  );

  useEffect(() => {
    void reload();
    // Sans abonnement (hors de Tauri), la liste reste simplement sans
    // rafraîchissement automatique.
    const unlisten = onWorkflowsChanged(() => void reload()).catch(() => undefined);
    return () => {
      void unlisten.then((stop) => stop?.()).catch(() => {});
    };
  }, [reload]);

  const move = (from: number, to: number) => {
    if (!workflows || from === to || to < 0 || to >= workflows.length) {
      return;
    }
    const reordered = arrayMove(workflows, from, to);
    setWorkflows(reordered);
    reorderWorkflows(reordered.map((workflow) => workflow.id))
      .then(setWorkflows)
      .catch(report);
  };

  const toggle = (workflow: Workflow) => {
    setWorkflowEnabled(workflow.id, !workflow.enabled)
      .then((updated) =>
        setWorkflows(
          (current) => current?.map((item) => (item.id === updated.id ? updated : item)) ?? null,
        ),
      )
      .catch(report);
  };

  const remove = (workflow: Workflow) => {
    deleteWorkflow(workflow.id)
      .then(() => {
        showToast("success", "workflows.deleted");
        setWorkflows((current) => current?.filter((item) => item.id !== workflow.id) ?? null);
      })
      .catch(report);
  };

  const createButton = (
    <Button
      variant="primary"
      onClick={() => startWizard({ mode: "create", draft: newDraft(), nameEdited: false })}
    >
      {t("workflows.create")}
    </Button>
  );
  const edit = (workflow: Workflow) =>
    startWizard({
      mode: "edit",
      workflowId: workflow.id,
      version: workflow.version,
      draft: draftOf(workflow),
      nameEdited: true,
    });

  return (
    <section className="section" aria-labelledby="section-title">
      <header className="section__header">
        <h1 id="section-title" className="section__title">
          {t("workflows.title")}
        </h1>
        {workflows && workflows.length > 0 && createButton}
      </header>
      {workflows === null ? (
        <Spinner />
      ) : workflows.length === 0 ? (
        <EmptyState
          title={t("workflows.empty.title")}
          description={t("workflows.empty.description")}
          action={createButton}
        />
      ) : (
        <>
          <p className="section__help">{t("workflows.orderHelp")}</p>
          <SortableWorkflows
            workflows={workflows}
            onMove={move}
            onToggle={toggle}
            onEdit={edit}
            onDelete={setToDelete}
          />
        </>
      )}
      <ConfirmDialog
        open={toDelete !== null}
        onOpenChange={(open) => {
          if (!open) {
            setToDelete(null);
          }
        }}
        title={t("workflows.deleteTitle", { name: toDelete?.name ?? "" })}
        description={t("workflows.deleteDescription")}
        confirmLabel={t("workflows.deleteConfirm")}
        destructive
        onConfirm={() => {
          if (toDelete) {
            remove(toDelete);
          }
        }}
      />
    </section>
  );
}

interface SortableWorkflowsProps {
  workflows: Workflow[];
  onMove: (from: number, to: number) => void;
  onToggle: (workflow: Workflow) => void;
  onEdit: (workflow: Workflow) => void;
  onDelete: (workflow: Workflow) => void;
}

function SortableWorkflows({
  workflows,
  onMove,
  onToggle,
  onEdit,
  onDelete,
}: SortableWorkflowsProps) {
  const { t } = useTranslation();
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
  const ids = workflows.map((workflow) => workflow.id);
  const describe = (id: string | number, index: number | undefined) => ({
    name: workflows.find((workflow) => workflow.id === id)?.name ?? "",
    position: (index ?? 0) + 1,
    total: workflows.length,
  });
  const indexOf = (id: string | number) => ids.indexOf(String(id));

  const onDragEnd = ({ active, over }: DragEndEvent) => {
    if (over) {
      onMove(indexOf(active.id), indexOf(over.id));
    }
  };

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragEnd={onDragEnd}
      accessibility={{
        screenReaderInstructions: { draggable: t("workflows.drag.instructions") },
        announcements: {
          onDragStart: ({ active }) =>
            t("workflows.drag.start", describe(active.id, indexOf(active.id))),
          onDragOver: ({ active, over }) =>
            over ? t("workflows.drag.over", describe(active.id, indexOf(over.id))) : undefined,
          onDragEnd: ({ active, over }) =>
            t("workflows.drag.end", describe(active.id, indexOf((over ?? active).id))),
          onDragCancel: ({ active }) =>
            t("workflows.drag.cancel", describe(active.id, indexOf(active.id))),
        },
      }}
    >
      <SortableContext items={ids} strategy={verticalListSortingStrategy}>
        <ol className="workflow-list" aria-label={t("workflows.list")}>
          {workflows.map((workflow, index) => (
            <WorkflowRow
              key={workflow.id}
              workflow={workflow}
              isFirst={index === 0}
              isLast={index === workflows.length - 1}
              onMoveUp={() => onMove(index, index - 1)}
              onMoveDown={() => onMove(index, index + 1)}
              onToggle={() => onToggle(workflow)}
              onEdit={() => onEdit(workflow)}
              onDelete={() => onDelete(workflow)}
            />
          ))}
        </ol>
      </SortableContext>
    </DndContext>
  );
}

interface WorkflowRowProps {
  workflow: Workflow;
  isFirst: boolean;
  isLast: boolean;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onToggle: () => void;
  onEdit: () => void;
  onDelete: () => void;
}

function WorkflowRow({
  workflow,
  isFirst,
  isLast,
  onMoveUp,
  onMoveDown,
  onToggle,
  onEdit,
  onDelete,
}: WorkflowRowProps) {
  const { t } = useTranslation();
  const {
    attributes,
    listeners,
    setNodeRef,
    setActivatorNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: workflow.id });
  const name = workflow.name;
  const classes = [
    "workflow-row",
    workflow.enabled ? "" : "workflow-row--disabled",
    isDragging ? "workflow-row--dragging" : "",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <li
      ref={setNodeRef}
      className={classes}
      style={{ transform: CSS.Transform.toString(transform), transition }}
    >
      <button
        type="button"
        ref={setActivatorNodeRef}
        className="workflow-row__handle"
        aria-label={t("workflows.dragHandle", { name })}
        {...attributes}
        {...listeners}
      >
        <span aria-hidden="true">⋮⋮</span>
      </button>
      <div className="workflow-row__body">
        <h2 className="workflow-row__name">{name}</h2>
        <p className="workflow-row__summary">
          <span className="workflow-row__label">{t("workflows.when")}</span>{" "}
          {describeTrigger(t, workflow.trigger)}
        </p>
        <p className="workflow-row__summary">
          <span className="workflow-row__label">{t("workflows.then")}</span>{" "}
          {describeActions(t, workflow.actions)}
        </p>
        {workflow.continue_on_match && (
          <p className="workflow-row__note">{t("workflows.continues")}</p>
        )}
      </div>
      <div className="workflow-row__controls">
        <Switch
          checked={workflow.enabled}
          onCheckedChange={onToggle}
          label={t("workflows.toggle", { name })}
        />
        <Button
          className="button--icon"
          aria-label={t("workflows.moveUp", { name })}
          disabled={isFirst}
          onClick={onMoveUp}
        >
          <span aria-hidden="true">↑</span>
        </Button>
        <Button
          className="button--icon"
          aria-label={t("workflows.moveDown", { name })}
          disabled={isLast}
          onClick={onMoveDown}
        >
          <span aria-hidden="true">↓</span>
        </Button>
        <Button aria-label={t("workflows.edit", { name })} onClick={onEdit}>
          {t("workflows.editShort")}
        </Button>
        <Button
          className="button--icon"
          variant="danger"
          aria-label={t("workflows.delete", { name })}
          onClick={onDelete}
        >
          <span aria-hidden="true">✕</span>
        </Button>
      </div>
    </li>
  );
}
