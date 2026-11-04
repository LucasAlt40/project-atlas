import { useDismiss } from '@/hooks/useDismiss';
import { useId, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { Icon } from '@/components/ui/Icon';
import { SegmentedControl } from '@/components/ui/SegmentedControl';
import { useT } from '@/i18n/I18nProvider';
import type { WorkspaceProjectDto } from '@/lib/tauri/commands';
import {
  allWorking,
  hasManyProjects,
  moved,
  movedTo,
  oneWorking,
  usageOf,
  withUse,
  type ProjectUse,
  type RunPlan,
} from '../model/runPlan';
import styles from './Projects.module.css';

interface Placement {
  vertical: 'above' | 'below';
  align: 'start' | 'end';
  maxHeight: number;
}

const DEFAULT_PLACEMENT: Placement = { vertical: 'below', align: 'start', maxHeight: 480 };
/** The panel's width (28rem) and the gap kept from the window's edge, in pixels. */
const PANEL_WIDTH = 448;
const EDGE = 12;

/** Opens toward the side with more room, and right-aligned when it would leave the window. */
function placementFor(anchor: HTMLElement | null): Placement {
  if (!anchor) return DEFAULT_PLACEMENT;
  const rect = anchor.getBoundingClientRect();
  const below = window.innerHeight - rect.bottom;
  const above = rect.top;
  const vertical = below >= above ? 'below' : 'above';
  return {
    vertical,
    align: rect.left + PANEL_WIDTH + EDGE > window.innerWidth ? 'end' : 'start',
    maxHeight: Math.max(160, Math.floor((vertical === 'below' ? below : above) - EDGE)),
  };
}

interface Props {
  projects: WorkspaceProjectDto[];
  plan: RunPlan;
  onChange: (plan: RunPlan) => void;
  disabled?: boolean;
}

/**
 * Which projects a workflow run changes (one run each, in order) and which it only consults
 * (ADR 0043). The projects that change are listed first, in the order of their runs, and can be
 * dragged by their handle (or moved with the arrow keys on it) to change that order. Shown only
 * for a workspace that has more than one project.
 */
export function RunProjectsPicker({ projects, plan, onChange, disabled = false }: Props) {
  const t = useT();
  const [open, setOpen] = useState(false);
  // Where the panel opens: on the side of the button with more room, never past the window.
  const [place, setPlace] = useState<Placement>(DEFAULT_PLACEMENT);
  // The project being dragged and where it would land; the list shows that order while dragging.
  const [drag, setDrag] = useState<{ id: string; over: number } | null>(null);
  const rows = useRef(new Map<string, HTMLLIElement>());
  const bodyId = useId();
  const root = useRef<HTMLDivElement>(null);
  useDismiss(open, root, () => {
    setOpen(false);
    setDrag(null);
  });
  if (projects.length < 2) return null;

  const workspace = { projects };
  const byId = (id: string) => projects.find((p) => p.id === id);
  const name = (id: string) => byId(id)?.name ?? id;
  const working = plan.workingProjectIds;
  const shown = drag ? movedTo(plan, drag.id, drag.over).workingProjectIds : working;
  const others = projects.filter((p) => !working.includes(p.id));
  const reorderable = working.length > 1;
  // Every change of the projects keeps the person's choice about waiting for each delivery.
  const change = (next: RunPlan) => {
    onChange(plan.waitForDelivery ? { ...next, waitForDelivery: true } : next);
  };
  const summary =
    working.length > 1
      ? t('runPlan.summary.chain', { order: working.map(name).join(' → ') })
      : t('runPlan.summary.one', { name: name(working[0] ?? '') });
  const badge =
    working.length > 1
      ? t('runPlan.badge.chain', { count: working.length })
      : t('runPlan.badge.one', { name: name(working[0] ?? '') });

  /** Where in the order a pointer at `y` would drop: before the first row whose middle is below it. */
  const dropIndex = (y: number) => {
    const order = working.filter((id) => id !== drag?.id);
    const index = order.findIndex((id) => {
      const rect = rows.current.get(id)?.getBoundingClientRect();
      return rect !== undefined && y < rect.top + rect.height / 2;
    });
    return index === -1 ? order.length : index;
  };
  const handleProps = (id: string) => ({
    onPointerDown: (event: PointerEvent<HTMLButtonElement>) => {
      if (disabled || event.button !== 0) return;
      event.preventDefault();
      event.currentTarget.setPointerCapture(event.pointerId);
      setDrag({ id, over: working.indexOf(id) });
    },
    onPointerMove: (event: PointerEvent<HTMLButtonElement>) => {
      if (drag?.id !== id) return;
      const over = dropIndex(event.clientY);
      if (over !== drag.over) setDrag({ id, over });
    },
    onPointerUp: () => {
      if (drag?.id === id) change(movedTo(plan, id, drag.over));
      setDrag(null);
    },
    onPointerCancel: () => {
      setDrag(null);
    },
    onKeyDown: (event: KeyboardEvent<HTMLButtonElement>) => {
      if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
        event.preventDefault();
        change(moved(plan, id, event.key === 'ArrowUp' ? -1 : 1));
      }
    },
  });

  const usageControl = (project: WorkspaceProjectDto) => {
    const use = usageOf(plan, project.id);
    const last = use === 'work' && working.length === 1;
    return (
      <SegmentedControl<ProjectUse>
        label={t('runPlan.useOf', { name: project.name })}
        value={use}
        segments={[
          { value: 'work', label: t('runPlan.use.work'), title: t('runPlan.use.work.hint') },
          {
            value: 'read',
            label: t('runPlan.use.read'),
            title: t('runPlan.use.read.hint'),
            disabled: last,
          },
          { value: 'none', label: t('runPlan.use.none'), disabled: last },
        ]}
        onChange={(next) => {
          change(withUse(plan, project.id, next, workspace));
        }}
      />
    );
  };
  const nameOf = (project: WorkspaceProjectDto) => (
    <span className={styles.planName}>
      <span className={styles.planTitle}>{project.name}</span>
      {project.role && (
        <span className={styles.planRole}>{t(`projects.role.${project.role}`)}</span>
      )}
    </span>
  );

  return (
    <div className={styles.scope} ref={root}>
      <button
        type="button"
        className={styles.scopeToggle}
        data-multi={working.length > 1}
        aria-expanded={open}
        aria-controls={bodyId}
        aria-label={`${t('runPlan.label')}: ${summary}`}
        title={summary}
        onClick={() => {
          if (!open) setPlace(placementFor(root.current));
          setOpen((o) => !o);
        }}
      >
        <Icon name="folder" size={15} />
        <span className={styles.scopeName}>{badge}</span>
        <Icon name="chevronDown" size={14} />
      </button>
      {open && (
        <fieldset
          id={bodyId}
          className={[styles.scopeBody, styles.planBody].join(' ')}
          data-placement={place.vertical}
          data-align={place.align}
          style={{ maxHeight: place.maxHeight }}
          disabled={disabled}
        >
          <legend className={styles.planLegend}>{t('runPlan.label')}</legend>
          <div className={styles.planHeader}>
            <span className={styles.planSection}>{t('runPlan.section.working')}</span>
            <span className={styles.planShortcuts}>
              <button
                type="button"
                className={styles.planLink}
                onClick={() => {
                  change(allWorking(workspace));
                }}
              >
                {t('runPlan.allWork')}
              </button>
              <button
                type="button"
                className={styles.planLink}
                onClick={() => {
                  change(oneWorking(workspace, working[0]));
                }}
              >
                {t('runPlan.oneWorks')}
              </button>
            </span>
          </div>
          <ol className={styles.planList} data-dragging={drag ? true : undefined}>
            {shown.map((id, index) => {
              const project = byId(id);
              if (!project) return null;
              return (
                <li
                  key={id}
                  ref={(element) => {
                    if (element) rows.current.set(id, element);
                    else rows.current.delete(id);
                  }}
                  className={styles.planRow}
                  data-dragged={drag?.id === id ? true : undefined}
                >
                  {reorderable ? (
                    <button
                      type="button"
                      className={styles.planHandle}
                      aria-label={t('runPlan.drag', { name: project.name })}
                      title={t('runPlan.drag', { name: project.name })}
                      {...handleProps(id)}
                    >
                      <Icon name="grip" size={16} />
                    </button>
                  ) : (
                    <span className={styles.planHandleSpace} aria-hidden="true" />
                  )}
                  <span className={styles.planOrder}>{index + 1}</span>
                  {nameOf(project)}
                  {usageControl(project)}
                </li>
              );
            })}
          </ol>
          {others.length > 0 && (
            <>
              <span className={styles.planSection}>{t('runPlan.section.others')}</span>
              <ul className={styles.planList}>
                {others.map((project) => (
                  <li key={project.id} className={styles.planRow} data-other>
                    {nameOf(project)}
                    {usageControl(project)}
                  </li>
                ))}
              </ul>
            </>
          )}
          {hasManyProjects(workspace) && (
            <p className={styles.muted}>{t('runPlan.many', { count: projects.length })}</p>
          )}
          {working.length > 1 && (
            <label className={styles.planOption}>
              <input
                type="checkbox"
                checked={plan.waitForDelivery === true}
                onChange={(event) => {
                  onChange({ ...plan, waitForDelivery: event.target.checked });
                }}
              />
              <span>
                <span className={styles.planTitle}>{t('runPlan.waitForDelivery')}</span>
                <span className={styles.muted}>{t('runPlan.waitForDelivery.hint')}</span>
              </span>
            </label>
          )}
          <p className={styles.planHint}>{t('runPlan.hint')}</p>
        </fieldset>
      )}
    </div>
  );
}
