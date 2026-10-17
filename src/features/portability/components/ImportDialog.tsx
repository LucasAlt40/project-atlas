import { useCallback, useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { Modal } from '@/components/ui/Modal';
import { useOptionalWorkspace } from '@/features/workspace/hooks/WorkspaceProvider';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import type { ImportPreviewDto, ImportReportDto, PortableKindDto } from '@/lib/tauri/commands';
import {
  asksToEdit,
  blockers,
  initialChoices,
  summarize,
  toRequest,
  uniqueBlockers,
  without,
  type Choices,
} from '../model/importPlan';
import { portabilityError } from '../model/messages';
import { importConfigs, pickConfigFile, previewConfigImport } from '../services/portabilityService';
import { ImportItemCard } from './ImportItemCard';
import { ImportResult } from './ImportResult';
import styles from './Portability.module.css';

interface Props {
  onClose: () => void;
  /** Called once the core has imported, so the screen can read what changed again. */
  onImported?: (report: ImportReportDto) => void;
}

type Stage =
  | { name: 'pick'; busy: boolean }
  | {
      name: 'review';
      file: string;
      text: string;
      preview: ImportPreviewDto;
      choices: Choices;
      busy: boolean;
    }
  | { name: 'done'; report: ImportReportDto };

const KIND_ORDER: PortableKindDto[] = ['workflow', 'workflow_template', 'agent', 'personality'];

/**
 * Importing a configuration file: pick it (in the operating system's own dialog), read what it
 * would do here, decide, and only then import. Nothing is written before the last button, and it
 * is written all together or not at all.
 */
export function ImportDialog({ onClose, onImported }: Props) {
  const { t } = useI18n();
  const workspace = useOptionalWorkspace();
  const workspaces = workspace?.workspaces ?? [];
  const [stage, setStage] = useState<Stage>({ name: 'pick', busy: false });
  const [error, setError] = useState<string | null>(null);

  const startWorkspace = workspace?.active?.id ?? null;

  const review = useCallback(
    async (file: string, text: string, workspaceId: string | null, keep?: Choices) => {
      const preview = await previewConfigImport(text, workspaceId);
      // Changing the workspace re-reads the file against it; what was decided about the rest stays.
      const choices = keep
        ? {
            ...initialChoices(preview, workspaceId),
            models: keep.models,
            bindings: { ...initialChoices(preview, workspaceId).bindings, ...keep.bindings },
            keepPermissions: keep.keepPermissions,
          }
        : initialChoices(preview, workspaceId);
      setStage({ name: 'review', file, text, preview, choices, busy: false });
    },
    [],
  );

  function choose() {
    setError(null);
    setStage({ name: 'pick', busy: true });
    pickConfigFile()
      .then(async (picked) => {
        if (!picked) {
          setStage({ name: 'pick', busy: false });
          return;
        }
        await review(picked.fileName, picked.text, startWorkspace);
      })
      .catch((cause: unknown) => {
        setError(portabilityError(t, cause));
        setStage({ name: 'pick', busy: false });
      });
  }

  function changeWorkspace(workspaceId: string | null) {
    if (stage.name !== 'review') return;
    const { file, text, choices } = stage;
    setError(null);
    setStage({ ...stage, busy: true });
    review(file, text, workspaceId, choices).catch((cause: unknown) => {
      setError(portabilityError(t, cause));
      setStage({ ...stage, busy: false });
    });
  }

  function run() {
    if (stage.name !== 'review') return;
    const current = stage;
    setError(null);
    setStage({ ...current, busy: true });
    importConfigs(current.text, toRequest(current.preview, current.choices))
      .then((report) => {
        setStage({ name: 'done', report });
        onImported?.(report);
      })
      .catch((cause: unknown) => {
        setError(portabilityError(t, cause));
        setStage({ ...current, busy: false });
      });
  }

  const label = t('portability.import.title');
  return (
    <Modal label={label} onClose={onClose} size={stage.name === 'review' ? 'wide' : 'analyze'}>
      <div className={styles.dialog}>
        <h2 className={styles.title}>{label}</h2>
        {stage.name === 'pick' && (
          <>
            <p className={styles.muted}>{t('portability.import.intro')}</p>
            <p className={styles.safety}>{t('portability.safety')}</p>
            <div className={styles.row}>
              <Button disabled={stage.busy} onClick={choose}>
                <Icon name="folder" size={14} />{' '}
                {t(stage.busy ? 'portability.import.reading' : 'portability.import.choose')}
              </Button>
            </div>
          </>
        )}
        {stage.name === 'review' && (
          <Review
            stage={stage}
            workspaces={workspaces.map((w) => ({ id: w.id, name: w.name }))}
            onChange={(change) => {
              setStage((current) =>
                current.name === 'review'
                  ? { ...current, choices: change(current.choices) }
                  : current,
              );
            }}
            onWorkspace={changeWorkspace}
            onImport={run}
            onClose={onClose}
            onAnother={choose}
          />
        )}
        {stage.name === 'done' && <ImportResult report={stage.report} onClose={onClose} />}
        {error && (
          <p role="alert" className={styles.error}>
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
}

interface ReviewProps {
  stage: Extract<Stage, { name: 'review' }>;
  workspaces: { id: string; name: string }[];
  onChange: (change: (current: Choices) => Choices) => void;
  onWorkspace: (id: string | null) => void;
  onImport: () => void;
  onClose: () => void;
  onAnother: () => void;
}

function Review({
  stage,
  workspaces,
  onChange,
  onWorkspace,
  onImport,
  onClose,
  onAnother,
}: ReviewProps) {
  const { t } = useI18n();
  const { preview, choices, file, busy } = stage;
  const problems = blockers(preview, choices);
  const summary = summarize(preview, choices);
  const items = [...preview.items].sort(
    (a, b) => KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind),
  );
  const nameOf = (ref: string) =>
    preview.items.find((i) => i.ref === ref)?.name ??
    preview.external.find((e) => e.ref === ref)?.name ??
    ref;

  return (
    <>
      <div className={styles.meta}>
        <Badge mono caps={false}>
          {t('portability.import.file', { file })}
        </Badge>
        {preview.source.appVersion && (
          <Badge mono caps={false}>
            {t('portability.import.from', { version: preview.source.appVersion })}
          </Badge>
        )}
        {preview.exportedAt !== null && (
          <Badge mono caps={false}>
            {new Date(preview.exportedAt).toLocaleString()}
          </Badge>
        )}
      </div>
      <p className={styles.safety}>{t('portability.safety')}</p>

      {preview.needsWorkspace && (
        <label className={styles.field}>
          {t('portability.import.workspace')}
          <select
            className={styles.control}
            value={choices.workspaceId ?? ''}
            disabled={busy}
            onChange={(e) => {
              onWorkspace(e.target.value || null);
            }}
          >
            <option value="">{t('portability.import.workspaceNone')}</option>
            {workspaces.map((w) => (
              <option key={w.id} value={w.id}>
                {w.name}
              </option>
            ))}
          </select>
          <span className={styles.helpText}>{t('portability.import.workspaceHelp')}</span>
        </label>
      )}

      {preview.external.length > 0 && (
        <section className={styles.section} aria-label={t('portability.external.title')}>
          <h3 className={styles.sectionHeading}>{t('portability.external.title')}</h3>
          <p className={styles.muted}>{t('portability.external.hint')}</p>
          {preview.external.map((external) => (
            <label key={external.ref} className={styles.field}>
              {t('portability.external.pick', {
                name: external.name,
                kind: t(`portability.kind.${external.kind}`),
              })}
              <select
                className={styles.control}
                value={choices.bindings[external.ref] ?? ''}
                onChange={(e) => {
                  const id = e.target.value;
                  onChange((current) => {
                    const rest = without(current.bindings, external.ref);
                    return { ...current, bindings: id ? { ...rest, [external.ref]: id } : rest };
                  });
                }}
              >
                <option value="">{t('portability.external.choose')}</option>
                {external.candidates.map((candidate) => (
                  <option key={candidate.id} value={candidate.id}>
                    {candidate.name}
                    {candidate.id === external.suggested
                      ? ` — ${t('portability.external.sameName')}`
                      : ''}
                  </option>
                ))}
              </select>
              {external.usedBy.length > 0 && (
                <span className={styles.helpText}>
                  {t('portability.external.usedBy', {
                    names: external.usedBy.map(nameOf).join(', '),
                  })}
                </span>
              )}
            </label>
          ))}
        </section>
      )}

      <section className={styles.section} aria-label={t('portability.import.items')}>
        <h3 className={styles.sectionHeading}>{t('portability.import.items')}</h3>
        <ul className={styles.list}>
          {items.map((item) => (
            <ImportItemCard
              key={item.ref}
              item={item}
              preview={preview}
              choices={choices}
              onChange={onChange}
            />
          ))}
        </ul>
      </section>

      {asksToEdit(preview, choices) && (
        <section className={styles.section} aria-label={t('portability.permissions.title')}>
          <h3 className={styles.sectionTitle}>{t('portability.permissions.title')}</h3>
          <label className={styles.check}>
            <input
              type="checkbox"
              checked={choices.keepPermissions}
              onChange={(e) => {
                onChange((current) => ({ ...current, keepPermissions: e.target.checked }));
              }}
            />
            <span>
              {t('portability.permissions.keep')}
              <span className={styles.helpText}>{t('portability.permissions.help')}</span>
            </span>
          </label>
        </section>
      )}

      <footer className={styles.footer}>
        {problems.length > 0 && (
          <ul className={styles.blockers} aria-label={t('portability.blockers')}>
            {uniqueBlockers(problems).map((problem) => (
              <li key={`${problem.code}-${'ref' in problem ? problem.ref : ''}`}>
                {t(`portability.blocker.${problem.code}` as TranslationKey, {
                  name: 'ref' in problem ? nameOf(problem.ref) : '',
                })}
              </li>
            ))}
          </ul>
        )}

        <div className={styles.footerRow}>
          <span className={styles.summary}>{t('portability.summary', { ...summary })}</span>
          <span className={styles.rowEnd}>
            <Button variant="ghost" disabled={busy} onClick={onAnother}>
              {t('portability.import.another')}
            </Button>
            <Button variant="secondary" onClick={onClose}>
              {t('common.cancel')}
            </Button>
            <Button disabled={busy || problems.length > 0} onClick={onImport}>
              {t(busy ? 'portability.import.working' : 'portability.import.action')}
            </Button>
          </span>
        </div>
      </footer>
    </>
  );
}
