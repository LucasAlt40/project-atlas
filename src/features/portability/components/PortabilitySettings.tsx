import { useEffect, useMemo, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useCatalog } from '@/features/agents/hooks/useCatalog';
import { listTemplates, listWorkflows } from '@/features/workflow/services/workflowService';
import { useOptionalWorkspace } from '@/features/workspace/hooks/WorkspaceProvider';
import { useI18n } from '@/i18n/I18nProvider';
import type { ExportTargetDto, PortableKindDto } from '@/lib/tauri/commands';
import { ExportDialog } from './ExportDialog';
import { ImportButton } from './PortabilityActions';
import styles from './Portability.module.css';

interface Group {
  kind: PortableKindDto;
  ids: string[];
}

/**
 * The one place to move everything at once: import a file, or export what was made here (a
 * backup, or a set to hand over). One configuration at a time is exported from its own card.
 */
export function PortabilitySettings() {
  const { t } = useI18n();
  const { catalog } = useCatalog();
  const workspace = useOptionalWorkspace();
  const activeId = workspace?.active?.id;
  const [templates, setTemplates] = useState<string[]>([]);
  const [workflows, setWorkflows] = useState<string[]>([]);
  const [choosing, setChoosing] = useState(false);
  const [targets, setTargets] = useState<ExportTargetDto[] | null>(null);
  const [kinds, setKinds] = useState<PortableKindDto[]>([
    'personality',
    'agent',
    'workflow_template',
    'workflow',
  ]);

  useEffect(() => {
    if (!choosing) return;
    let stale = false;
    listTemplates()
      .then((list) => {
        if (!stale) setTemplates(list.filter((x) => x.kind === 'custom').map((x) => x.id));
      })
      .catch(() => {
        if (!stale) setTemplates([]);
      });
    if (activeId) {
      listWorkflows(activeId)
        .then((list) => {
          if (!stale) setWorkflows(list.map((w) => w.id));
        })
        .catch(() => {
          if (!stale) setWorkflows([]);
        });
    }
    return () => {
      stale = true;
    };
  }, [choosing, activeId]);

  const groups = useMemo<Group[]>(
    () => [
      {
        kind: 'personality',
        ids:
          catalog.status === 'ready'
            ? catalog.personalities.filter((p) => p.source === 'custom').map((p) => p.id)
            : [],
      },
      { kind: 'agent', ids: catalog.status === 'ready' ? catalog.agents.map((a) => a.id) : [] },
      { kind: 'workflow_template', ids: templates },
      { kind: 'workflow', ids: workflows },
    ],
    [catalog, templates, workflows],
  );
  const chosen = groups.filter((g) => kinds.includes(g.kind) && g.ids.length > 0);
  const total = chosen.reduce((sum, g) => sum + g.ids.length, 0);

  return (
    <div className={styles.section}>
      <p className={styles.muted}>{t('portability.settings.help')}</p>
      <div className={styles.cards}>
        <div className={styles.actionCard}>
          <h3 className={styles.cardTitle}>
            <Icon name="download" size={16} /> {t('portability.settings.importCard')}
          </h3>
          <div className={styles.row}>
            <ImportButton />
          </div>
        </div>
        <div className={styles.actionCard}>
          <h3 className={styles.cardTitle}>
            <Icon name="upload" size={16} /> {t('portability.settings.exportCard')}
          </h3>
          <div className={styles.row}>
            <Button
              variant="secondary"
              onClick={() => {
                setChoosing(true);
              }}
            >
              <Icon name="upload" size={14} /> {t('portability.settings.exportAll')}
            </Button>
          </div>
        </div>
      </div>
      {choosing && !targets && (
        <fieldset className={styles.chooser}>
          <legend className={styles.sectionTitle}>{t('portability.settings.choose')}</legend>
          {groups.map((group) => (
            <label key={group.kind} className={styles.check}>
              <input
                type="checkbox"
                checked={kinds.includes(group.kind)}
                disabled={group.ids.length === 0}
                onChange={(e) => {
                  setKinds((current) =>
                    e.target.checked
                      ? [...current, group.kind]
                      : current.filter((k) => k !== group.kind),
                  );
                }}
              />
              <span>{t(`portability.settings.group.${group.kind}`, { n: group.ids.length })}</span>
            </label>
          ))}
          {!activeId && <p className={styles.muted}>{t('portability.settings.noWorkspace')}</p>}
          <div className={styles.row}>
            <Button
              disabled={total === 0}
              onClick={() => {
                setTargets(chosen.flatMap((g) => g.ids.map((id) => ({ kind: g.kind, id }))));
              }}
            >
              {t('portability.settings.continue', { n: total })}
            </Button>
            <Button
              variant="ghost"
              onClick={() => {
                setChoosing(false);
              }}
            >
              {t('common.cancel')}
            </Button>
          </div>
        </fieldset>
      )}
      {targets && (
        <ExportDialog
          targets={targets}
          title={t('portability.export.titleSet')}
          onClose={() => {
            setTargets(null);
            setChoosing(false);
          }}
        />
      )}
    </div>
  );
}
