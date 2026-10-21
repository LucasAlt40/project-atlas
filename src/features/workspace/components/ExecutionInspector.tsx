import { Modal } from '@/components/ui/Modal';
import { useI18n } from '@/i18n/I18nProvider';
import { shortId } from '../model/inspection';
import { ExecutionView, type ExecutionViewProps } from './ExecutionView';
import styles from './Inspector.module.css';

/**
 * An execution opened from outside its agent's card (a workflow's timeline): the same view as
 * the card's detail, in a dialog over the screen it was opened from.
 */
export function ExecutionInspector({
  onClose,
  ...view
}: ExecutionViewProps & { onClose: () => void }) {
  const { t } = useI18n();
  return (
    <Modal
      label={t('inspector.title', { id: shortId(view.execution.id) })}
      onClose={onClose}
      size="xl"
    >
      <div className={styles.inspectorFrame}>
        <ExecutionView {...view} />
      </div>
    </Modal>
  );
}
