import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from 'react';
import type { InteractionAnswerDto, WorkflowModeDto } from '@/lib/tauri/commands';
import { addAgentNode, addConditionNode, addEndNode, asCustom } from '../model/edit';
import {
  answerInteraction,
  applyChanges,
  cancelWorkflow,
  createFromTemplate,
  createWorkflow,
  deleteWorkflow,
  discardChanges,
  getRecovery,
  getRun,
  keepChanges,
  listIdes,
  openInIde,
  listRuns,
  listTemplates,
  listWorkflows,
  pauseWorkflow,
  resumeWorkflow,
  selectTemplate,
  startWorkflow,
  subscribeToWorkflowEvents,
  updateWorkflow,
  validateWorkflow,
} from '../services/workflowService';
import { isActiveRun } from '../types';
import type {
  Ide,
  RecoveryPlan,
  ValidationReport,
  Workflow,
  WorkflowEvent,
  WorkflowRun,
  WorkflowTemplate,
} from '../types';

/** Undo/redo over the draft of one workflow. The saved version is the reference for "dirty". */
interface Draft {
  saved: Workflow;
  past: Workflow[];
  present: Workflow;
  future: Workflow[];
}

type DraftAction =
  | { type: 'open'; workflow: Workflow }
  | { type: 'edit'; id: string; change: (workflow: Workflow) => Workflow }
  | { type: 'undo'; id: string }
  | { type: 'redo'; id: string }
  | { type: 'saved'; workflow: Workflow }
  | { type: 'forget'; id: string };

const MAX_HISTORY = 100;

function draftsReducer(state: Record<string, Draft>, action: DraftAction): Record<string, Draft> {
  switch (action.type) {
    case 'open': {
      const existing = state[action.workflow.id];
      // A draft with unsaved changes survives a re-read of the saved version.
      if (existing && existing.present !== existing.saved) {
        return { ...state, [action.workflow.id]: { ...existing, saved: action.workflow } };
      }
      return {
        ...state,
        [action.workflow.id]: {
          saved: action.workflow,
          past: [],
          present: action.workflow,
          future: [],
        },
      };
    }
    case 'edit': {
      const draft = state[action.id];
      if (!draft) return state;
      const next = action.change(draft.present);
      if (next === draft.present) return state;
      return {
        ...state,
        [action.id]: {
          ...draft,
          past: [...draft.past, draft.present].slice(-MAX_HISTORY),
          present: next,
          future: [],
        },
      };
    }
    case 'undo': {
      const draft = state[action.id];
      const previous = draft?.past[draft.past.length - 1];
      if (!draft || !previous) return state;
      return {
        ...state,
        [action.id]: {
          ...draft,
          past: draft.past.slice(0, -1),
          present: previous,
          future: [draft.present, ...draft.future],
        },
      };
    }
    case 'redo': {
      const draft = state[action.id];
      const next = draft?.future[0];
      if (!draft || !next) return state;
      return {
        ...state,
        [action.id]: {
          ...draft,
          past: [...draft.past, draft.present],
          present: next,
          future: draft.future.slice(1),
        },
      };
    }
    case 'saved':
      return {
        ...state,
        [action.workflow.id]: {
          saved: action.workflow,
          past: [],
          present: action.workflow,
          future: [],
        },
      };
    case 'forget':
      return Object.fromEntries(Object.entries(state).filter(([id]) => id !== action.id));
  }
}

/** How reading a workspace's workflows ended, and for which workspace. */
interface Loaded {
  workspaceId: string;
  error: unknown;
  failed: boolean;
}

export type LoadState =
  { status: 'loading' } | { status: 'error'; error: unknown } | { status: 'ready' };

/**
 * Workflows of one workspace: their definitions (with an editable draft each), their runs and
 * how the run being shown is going. The runs are driven by the core, not by this hook: leaving
 * the screen (or the workspace) does not stop one, and coming back reads it again.
 */
export function useWorkflows(workspaceId: string | undefined) {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [workflows, setWorkflows] = useState<Workflow[]>([]);
  const [templates, setTemplates] = useState<WorkflowTemplate[]>([]);
  const [runs, setRuns] = useState<WorkflowRun[]>([]);
  const [drafts, dispatch] = useReducer(draftsReducer, {});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  /** The run drawn on the canvas; `null` shows the editor. */
  const [shownRunId, setShownRunId] = useState<string | null>(null);
  const [checked, setChecked] = useState<{ of: Workflow; report: ValidationReport | null } | null>(
    null,
  );
  const [error, setError] = useState<unknown>(undefined);
  const [ides, setIdes] = useState<Ide[]>([]);
  const [integrating, setIntegrating] = useState(false);
  const [answering, setAnswering] = useState(false);
  /** Why the core refused the last answer, by the question it was for. */
  const [answerError, setAnswerError] = useState<{ id: string; error: unknown }>();

  useEffect(() => {
    let cancelled = false;
    listTemplates()
      .then((list) => {
        if (!cancelled) setTemplates(list);
      })
      .catch(() => {
        // Without templates the user can still build a workflow by hand.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // The editors Atlas can open a worktree in, found once.
  useEffect(() => {
    let cancelled = false;
    listIdes()
      .then((list) => {
        if (!cancelled) setIdes(list);
      })
      .catch(() => {
        // No editor to offer: the button says so.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Everything of the workspace is read again when it is shown.
  useEffect(() => {
    if (!workspaceId) return;
    let cancelled = false;
    Promise.all([listWorkflows(workspaceId), listRuns(workspaceId)])
      .then(([list, runList]) => {
        if (cancelled) return;
        setWorkflows(list);
        setRuns(runList);
        for (const workflow of list) dispatch({ type: 'open', workflow });
        const active = runList.find((run) => isActiveRun(run));
        setSelectedId((current) =>
          current && list.some((w) => w.id === current)
            ? current
            : (active?.workflowId ?? list[0]?.id ?? null),
        );
        setShownRunId(active?.id ?? null);
        setLoaded({ workspaceId, error: undefined, failed: false });
      })
      .catch((reason: unknown) => {
        if (!cancelled) setLoaded({ workspaceId, error: reason, failed: true });
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceId]);

  // Live: an event says a run changed; the run is read again (events carry ids, not state).
  const pending = useRef(new Set<string>());
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const refreshRuns = useCallback(() => {
    timer.current = undefined;
    const ids = [...pending.current];
    pending.current.clear();
    for (const id of ids) {
      getRun(id)
        .then((run) => {
          if (!run) return;
          setRuns((current) =>
            current.some((r) => r.id === run.id)
              ? current.map((r) => (r.id === run.id ? run : r))
              : [run, ...current],
          );
        })
        .catch(() => {
          // A missed update is repaired by the next event, or by reopening the screen.
        });
    }
  }, []);
  const onEvent = useCallback(
    (event: WorkflowEvent) => {
      if (event.workspaceId !== workspaceId) return;
      pending.current.add(event.executionId);
      timer.current ??= setTimeout(refreshRuns, 40);
    },
    [workspaceId, refreshRuns],
  );
  useEffect(() => {
    let stop: (() => void) | undefined;
    let disposed = false;
    subscribeToWorkflowEvents(onEvent)
      .then((unlisten) => {
        if (disposed) unlisten();
        else stop = unlisten;
      })
      .catch(() => {
        // Live updates are best-effort.
      });
    return () => {
      disposed = true;
      stop?.();
      if (timer.current) clearTimeout(timer.current);
      timer.current = undefined;
    };
  }, [onEvent]);

  // Loading until the workspace on screen has been read.
  const load: LoadState =
    !loaded || loaded.workspaceId !== workspaceId
      ? { status: 'loading' }
      : loaded.failed
        ? { status: 'error', error: loaded.error }
        : { status: 'ready' };

  const selected = workflows.find((w) => w.id === selectedId);
  const draft = selectedId ? drafts[selectedId] : undefined;
  const runsOfSelected = useMemo(
    () => runs.filter((run) => run.workflowId === selectedId),
    [runs, selectedId],
  );
  const activeRun = runsOfSelected.find((run) => isActiveRun(run));
  const shownRun = runs.find((run) => run.id === shownRunId && run.workflowId === selectedId);

  // A run that failed says where it would go on from (read again when the run or the saved
  // workflow changes: the plan is worked out under the workflow as it is saved).
  const [recovery, setRecovery] = useState<{ of: string; plan: RecoveryPlan | null }>();
  const failedRunId = shownRun?.status === 'failed' ? shownRun.id : undefined;
  const failedRunStamp = shownRun?.updatedAt;
  const savedWorkflow = draft?.saved;
  useEffect(() => {
    if (!failedRunId) return;
    let cancelled = false;
    getRecovery(failedRunId)
      .then((plan) => {
        if (!cancelled) setRecovery({ of: failedRunId, plan });
      })
      .catch(() => {
        if (!cancelled) setRecovery({ of: failedRunId, plan: null });
      });
    return () => {
      cancelled = true;
    };
  }, [failedRunId, failedRunStamp, savedWorkflow]);

  // The editor checks the draft with the core as it changes.
  const present = draft?.present;
  useEffect(() => {
    if (!present) return;
    let cancelled = false;
    const handle = setTimeout(() => {
      validateWorkflow(present)
        .then((report) => {
          if (!cancelled) setChecked({ of: present, report });
        })
        .catch(() => {
          if (!cancelled) setChecked({ of: present, report: null });
        });
    }, 120);
    return () => {
      cancelled = true;
      clearTimeout(handle);
    };
  }, [present]);

  // A report belongs to the draft it was made for; an edited draft has none until it is checked.
  const validation = checked && checked.of === present ? checked.report : null;

  const upsert = useCallback((workflow: Workflow) => {
    setWorkflows((current) =>
      current.some((w) => w.id === workflow.id)
        ? current.map((w) => (w.id === workflow.id ? workflow : w))
        : [...current, workflow],
    );
  }, []);

  const guard = useCallback(async <T>(action: () => Promise<T>): Promise<T | undefined> => {
    setError(undefined);
    try {
      return await action();
    } catch (reason) {
      setError(reason);
      return undefined;
    }
  }, []);

  const open = useCallback((id: string) => {
    setSelectedId(id);
    setShownRunId(null);
    setError(undefined);
  }, []);

  const edit = useCallback(
    (change: (workflow: Workflow) => Workflow) => {
      if (selectedId && !activeRun) dispatch({ type: 'edit', id: selectedId, change });
    },
    [selectedId, activeRun],
  );

  const create = useCallback(
    async (templateId: string | null, mode: WorkflowModeDto, name: string) => {
      if (!workspaceId) return undefined;
      return guard(async () => {
        let workflow: Workflow;
        let missing: string[] = [];
        if (templateId) {
          const built = await createFromTemplate(workspaceId, templateId, mode, name || undefined);
          workflow = built.workflow;
          missing = built.missingRoles;
        } else {
          workflow = await createWorkflow({ workspaceId, name: name || 'Workflow', mode });
        }
        upsert(workflow);
        dispatch({ type: 'open', workflow });
        setSelectedId(workflow.id);
        setShownRunId(null);
        return { workflow, missing };
      });
    },
    [workspaceId, guard, upsert],
  );

  const save = useCallback(async () => {
    if (!draft) return undefined;
    return guard(async () => {
      const saved = await updateWorkflow(draft.present);
      upsert(saved);
      dispatch({ type: 'saved', workflow: saved });
      return saved;
    });
  }, [draft, guard, upsert]);

  const remove = useCallback(async () => {
    if (!selectedId) return;
    await guard(async () => {
      await deleteWorkflow(selectedId);
      setWorkflows((current) => current.filter((w) => w.id !== selectedId));
      dispatch({ type: 'forget', id: selectedId });
      setSelectedId(workflows.find((w) => w.id !== selectedId)?.id ?? null);
      setShownRunId(null);
    });
  }, [selectedId, workflows, guard]);

  const run = useCallback(
    async (task: string) => {
      if (!draft) return undefined;
      return guard(async () => {
        // What is run is what is saved: unsaved edits are saved first.
        let workflow = draft.saved;
        if (draft.present !== draft.saved) {
          workflow = await updateWorkflow(draft.present);
          upsert(workflow);
          dispatch({ type: 'saved', workflow });
        }
        const started = await startWorkflow(workflow.id, task);
        setRuns((current) => [started, ...current.filter((r) => r.id !== started.id)]);
        setShownRunId(started.id);
        return started;
      });
    },
    [draft, guard, upsert],
  );

  const control = useCallback(
    async (action: 'pause' | 'resume' | 'cancel') => {
      const target = shownRun ?? activeRun;
      if (!target) return;
      await guard(async () => {
        if (action === 'resume' && target.status === 'failed' && draft) {
          // A run goes on under the workflow as it is saved: edits made to fix it are saved first.
          if (draft.present !== draft.saved) {
            const saved = await updateWorkflow(draft.present);
            upsert(saved);
            dispatch({ type: 'saved', workflow: saved });
          }
        }
        if (action === 'pause') await pauseWorkflow(target.id);
        else if (action === 'resume') await resumeWorkflow(target.id);
        else await cancelWorkflow(target.id);
        const fresh = await getRun(target.id);
        if (fresh) setRuns((current) => current.map((r) => (r.id === fresh.id ? fresh : r)));
      });
    },
    [shownRun, activeRun, guard, draft, upsert],
  );

  /** The user's decision about the run's code. What comes back is the run as the core now has it. */
  const integrate = useCallback(
    async (decision: 'apply' | 'keep' | 'discard') => {
      const target = shownRun ?? activeRun;
      if (!target) return;
      setIntegrating(true);
      await guard(async () => {
        const act = { apply: applyChanges, keep: keepChanges, discard: discardChanges }[decision];
        const next = await act(target.id);
        setRuns((current) => current.map((r) => (r.id === next.id ? next : r)));
      });
      setIntegrating(false);
    },
    [shownRun, activeRun, guard],
  );

  /**
   * The person's answer to a question a step asked. The core checks it and the step goes on; a
   * refused answer (the question is gone, or it is not one of its options) is kept to be shown
   * next to the question.
   */
  const answer = useCallback(
    async (interactionId: string, reply: InteractionAnswerDto) => {
      const target = shownRun ?? activeRun;
      if (!target) return;
      setAnswering(true);
      setAnswerError(undefined);
      try {
        await answerInteraction(target.id, interactionId, reply);
        const fresh = await getRun(target.id);
        if (fresh) setRuns((current) => current.map((r) => (r.id === fresh.id ? fresh : r)));
      } catch (reason) {
        setAnswerError({ id: interactionId, error: reason });
      }
      setAnswering(false);
    },
    [shownRun, activeRun],
  );

  const openRunInIde = useCallback(
    async (ideId: string) => {
      const target = shownRun ?? activeRun;
      if (!target) return;
      await guard(() => openInIde(target.id, ideId));
    },
    [shownRun, activeRun, guard],
  );

  return {
    ides,
    integrating,
    answering,
    answerError,
    answer,
    integrate,
    openRunInIde,
    load,
    workflows,
    templates,
    runs: runsOfSelected,
    selected,
    draft: draft?.present,
    dirty: draft ? draft.present !== draft.saved : false,
    canUndo: (draft?.past.length ?? 0) > 0,
    canRedo: (draft?.future.length ?? 0) > 0,
    validation,
    activeRun,
    shownRun,
    recovery: recovery && recovery.of === shownRun?.id ? recovery.plan : null,
    error,
    clearError: () => {
      setError(undefined);
    },
    open,
    edit,
    undo: () => {
      if (selectedId) dispatch({ type: 'undo', id: selectedId });
    },
    redo: () => {
      if (selectedId) dispatch({ type: 'redo', id: selectedId });
    },
    create,
    save,
    remove,
    run,
    pause: () => control('pause'),
    resume: () => control('resume'),
    cancel: () => control('cancel'),
    showRun: setShownRunId,
    customize: () => {
      edit(asCustom);
    },
    recommend: selectTemplate,
    addAgent: (agentId: string, label: string) => {
      edit((w) => addAgentNode(w, agentId, label));
    },
    addCondition: (label: string) => {
      edit((w) => addConditionNode(w, label));
    },
    addEnd: (label: string) => {
      edit((w) => addEndNode(w, label));
    },
  };
}

export type WorkflowSpace = ReturnType<typeof useWorkflows>;
