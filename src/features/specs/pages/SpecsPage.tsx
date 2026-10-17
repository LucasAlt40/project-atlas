import { useMemo, useState } from 'react';
import { useNavigation } from '@/app/NavigationContext';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useCatalog } from '@/features/agents/hooks/useCatalog';
import { useWorkspace } from '@/features/workspace/hooks/WorkspaceProvider';
import type { Workspace } from '@/features/workspace/types';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import { ImportDialog } from '../components/ImportDialog';
import { NewSpecDialog } from '../components/NewSpecDialog';
import { Overview } from '../components/Overview';
import { SpecDetail } from '../components/SpecDetail';
import { useSpecs } from '../hooks/useSpecs';
import styles from '../components/Specs.module.css';

/**
 * Specs: write the specification before the code. A request becomes a proposal, requirements,
 * a design and a task list that agents write and a person approves one by one, then a review of
 * the whole; only then can a workflow build from it (ADR 0035).
 */
export function SpecsPage() {
  const { t } = useI18n();
  const workspace = useWorkspace();
  if (!workspace.active) {
    return <p className={styles.muted}>{t('specs.noWorkspace')}</p>;
  }
  return <Home key={workspace.active.id} workspace={workspace.active} />;
}

function Home({ workspace }: { workspace: Workspace }) {
  const { t } = useI18n();
  const navigation = useNavigation();
  const { catalog } = useCatalog();
  const agents = useMemo(() => (catalog.status === 'ready' ? catalog.agents : []), [catalog]);
  const { state } = useSpecs(workspace.id);
  const intent = navigation.intent;
  const [openId, setOpenId] = useState<string | null>(
    intent?.type === 'open-spec' ? intent.specId : null,
  );
  const [dialog, setDialog] = useState<'new' | 'import' | null>(
    intent?.type === 'new-spec' ? 'new' : null,
  );

  const specs = state.status === 'ready' ? state.specs : [];

  return (
    <div className={styles.page}>
      {openId ? (
        <h1 className={styles.srOnly}>{t('nav.specs')}</h1>
      ) : (
        <header className={styles.pageHead}>
          <div className={styles.heading}>
            <h1>{t('nav.specs')}</h1>
            <p>{t('specs.subtitle')}</p>
          </div>
          {state.status === 'ready' && specs.length > 0 && (
            <div className={styles.headRight}>
              <Button
                variant="secondary"
                onClick={() => {
                  setDialog('import');
                }}
              >
                <Icon name="download" size={14} />
                {t('specs.import')}
              </Button>
              <Button
                onClick={() => {
                  setDialog('new');
                }}
              >
                <Icon name="plus" size={14} />
                {t('specs.new')}
              </Button>
            </div>
          )}
        </header>
      )}

      {openId ? (
        <SpecDetail
          key={openId}
          workspace={workspace}
          agents={agents}
          specId={openId}
          onBack={() => {
            setOpenId(null);
          }}
          onDeleted={() => {
            setOpenId(null);
          }}
          onImplement={(specId) => {
            navigation.navigate('workflow', { type: 'workflow-from-spec', specId });
          }}
        />
      ) : state.status === 'loading' ? (
        <p className={styles.muted} role="status">
          {t('specs.loading')}
        </p>
      ) : state.status === 'error' ? (
        <p className={styles.error} role="alert">
          {errorMessage(t, state.error)}
        </p>
      ) : (
        <Overview
          specs={specs}
          agents={agents}
          onOpen={setOpenId}
          onNew={() => {
            setDialog('new');
          }}
          onImport={() => {
            setDialog('import');
          }}
        />
      )}

      {dialog === 'new' && (
        <NewSpecDialog
          workspace={workspace}
          agents={agents}
          onClose={() => {
            setDialog(null);
          }}
          onCreated={(view) => {
            setDialog(null);
            setOpenId(view.spec.id);
          }}
        />
      )}
      {dialog === 'import' && (
        <ImportDialog
          workspace={workspace}
          agents={agents}
          onClose={() => {
            setDialog(null);
          }}
          onImported={(view) => {
            setDialog(null);
            setOpenId(view.spec.id);
          }}
        />
      )}
    </div>
  );
}
