import { Button } from '@/components/ui/Button';
import { Callout } from '@/components/ui/Callout';
import { Modal } from '@/components/ui/Modal';
import { useT } from '@/i18n/I18nProvider';
import type { WorkspaceProjectDto } from '@/lib/tauri/commands';
import {
  hasManyProjects,
  isChain,
  suggestedPlan,
  type RunPlan,
} from '@/features/workspace/model/runPlan';
import styles from './Workflow.module.css';

interface Props {
  projects: WorkspaceProjectDto[];
  plan: RunPlan;
  /** Started from an approved spec, not from a free task. */
  fromSpec: boolean;
  onCancel: () => void;
  /** Runs with the plan as it is. */
  onConfirm: () => void;
  /** Runs with Atlas's suggestion instead (one project changed, the rest consulted). */
  onUseSuggestion: (plan: RunPlan) => void;
}

/**
 * Said before every run in a workspace of several projects (ADR 0043): which projects will be
 * changed, in what order and as how many runs, and which are only consulted. With many projects
 * changed it says what Atlas suggests instead; the person decides.
 */
export function RunPlanConfirm({
  projects,
  plan,
  fromSpec,
  onCancel,
  onConfirm,
  onUseSuggestion,
}: Props) {
  const t = useT();
  const name = (id: string) => projects.find((p) => p.id === id)?.name ?? id;
  const working = plan.workingProjectIds;
  const chain = isChain(plan);
  const untouched = projects.filter(
    (p) => !working.includes(p.id) && !plan.readProjectIds.includes(p.id),
  );
  const many = hasManyProjects({ projects });
  const suggestion = suggestedPlan({ projects });
  const suggestDifferent = many && chain && suggestion.workingProjectIds.join() !== working.join();

  return (
    <Modal
      label={t('runPlan.confirm.title')}
      title={t('runPlan.confirm.title')}
      size="md"
      onClose={onCancel}
      footer={
        <>
          <Button variant="secondary" onClick={onCancel}>
            {t('common.cancel')}
          </Button>
          {suggestDifferent && (
            <Button
              variant="secondary"
              onClick={() => {
                onUseSuggestion(suggestion);
              }}
            >
              {t('runPlan.confirm.useSuggestion', {
                name: name(suggestion.workingProjectIds[0] ?? ''),
              })}
            </Button>
          )}
          <Button onClick={onConfirm}>
            {chain
              ? t('runPlan.confirm.runChain', { count: working.length })
              : t('runPlan.confirm.run')}
          </Button>
        </>
      }
    >
      <div className={styles.planConfirm}>
        <p>
          {fromSpec
            ? t('runPlan.confirm.introSpec', { count: projects.length })
            : t('runPlan.confirm.intro', { count: projects.length })}
        </p>
        <div>
          <h3 className={styles.planHeading}>
            {chain ? t('runPlan.confirm.changesChain') : t('runPlan.confirm.changesOne')}
          </h3>
          <ol className={styles.planSteps}>
            {working.map((id) => (
              <li key={id}>{name(id)}</li>
            ))}
          </ol>
        </div>
        {plan.readProjectIds.length > 0 && (
          <p>
            <strong>{t('runPlan.confirm.reads')}</strong> {plan.readProjectIds.map(name).join(', ')}
          </p>
        )}
        {untouched.length > 0 && (
          <p>
            <strong>{t('runPlan.confirm.untouched')}</strong>{' '}
            {untouched.map((p) => p.name).join(', ')}
          </p>
        )}
        {chain ? (
          <Callout tone="info">{t('runPlan.confirm.chainNote')}</Callout>
        ) : (
          <Callout tone="info">
            {t('runPlan.confirm.oneNote', { name: name(working[0] ?? '') })}
          </Callout>
        )}
        {suggestDifferent && (
          <Callout
            tone="warning"
            title={t('runPlan.confirm.manyTitle', { count: projects.length })}
          >
            {t('runPlan.confirm.many', {
              count: working.length,
              name: name(suggestion.workingProjectIds[0] ?? ''),
            })}
          </Callout>
        )}
      </div>
    </Modal>
  );
}
