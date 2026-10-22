import { useMemo, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Callout } from '@/components/ui/Callout';
import { OverflowMenu } from '@/components/ui/OverflowMenu';
import { errorMessage } from '@/i18n/messages';
import { Icon } from '@/components/ui/Icon';
import { SegmentedControl } from '@/components/ui/SegmentedControl';
import { useNow } from '@/features/agents/hooks/useNow';
import type { Agent } from '@/features/agents/types';
import { useI18n } from '@/i18n/I18nProvider';
import { countsByGroup, GROUPS, inGroup, type Group } from '../model/status';
import type { SpecSummary } from '../types';
import { DeleteSpecDialog } from './DeleteSpecDialog';
import { SpecCard } from './SpecCard';
import styles from './Specs.module.css';

interface Props {
  specs: SpecSummary[];
  agents: Agent[];
  onOpen: (specId: string) => void;
  onNew: () => void;
  onImport: () => void;
  /** Archives a published spec; resolves when the core has done it. */
  onArchive: (specId: string) => Promise<void>;
  /** Removes a spec that built nothing from Atlas. */
  onDelete: (specId: string) => Promise<void>;
}

/** The workspace's specs: what is going on, what needs the person, what is ready to build. */
export function Overview({ specs, agents, onOpen, onNew, onImport, onArchive, onDelete }: Props) {
  const { t } = useI18n();
  const [deleting, setDeleting] = useState<SpecSummary | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const run = (work: Promise<void>) => {
    setFailure(null);
    work.catch((error: unknown) => {
      setFailure(errorMessage(t, error));
    });
  };
  const now = useNow();
  const [group, setGroup] = useState<Group>('all');
  const [query, setQuery] = useState('');
  const counts = useMemo(() => countsByGroup(specs), [specs]);
  const shown = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return specs
      .filter((spec) => inGroup(spec, group))
      .filter(
        (spec) =>
          !needle ||
          spec.title.toLowerCase().includes(needle) ||
          spec.changeId.toLowerCase().includes(needle) ||
          spec.request.toLowerCase().includes(needle),
      );
  }, [specs, group, query]);

  if (specs.length === 0) {
    return (
      <div className={styles.empty} role="status">
        <Icon name="specs" size={28} />
        <h2>{t('specs.empty.title')}</h2>
        <p>{t('specs.empty.body')}</p>
        <div className={styles.row}>
          <Button onClick={onNew}>{t('specs.new')}</Button>
          <Button variant="secondary" onClick={onImport}>
            {t('specs.import')}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <section className={styles.overview} aria-label={t('specs.list')}>
      <div className={styles.listControls}>
        <label className={styles.search}>
          <Icon name="search" size={14} />
          <input
            type="search"
            value={query}
            placeholder={t('specs.search')}
            aria-label={t('specs.search')}
            onChange={(event) => {
              setQuery(event.target.value);
            }}
          />
        </label>
        <SegmentedControl
          label={t('specs.filter')}
          value={group}
          onChange={setGroup}
          segments={GROUPS.map((name) => ({
            value: name,
            label: `${t(`specs.group.${name}`)} (${String(counts[name])})`,
          }))}
        />
      </div>
      {shown.length === 0 ? (
        <p className={styles.muted} role="status">
          {t('specs.noMatches')}
        </p>
      ) : (
        <ul className={styles.specList}>
          {shown.map((spec) => (
            <li key={spec.id} className={styles.specItem}>
              <SpecCard
                spec={spec}
                now={now}
                agentName={agents.find((a) => a.id === spec.authorAgentId)?.name ?? '—'}
                onOpen={onOpen}
              />
              {(spec.status === 'published' || spec.implementations === 0) && (
                <span className={styles.specMenu}>
                  <OverflowMenu
                    label={t('specs.moreActions', { title: spec.title })}
                    items={[
                      ...(spec.status === 'published'
                        ? [
                            {
                              label: t('specs.archive'),
                              icon: 'stack' as const,
                              onSelect: () => {
                                run(onArchive(spec.id));
                              },
                            },
                          ]
                        : []),
                      ...(spec.implementations === 0
                        ? [
                            {
                              label: t('common.delete'),
                              icon: 'trash' as const,
                              danger: true,
                              onSelect: () => {
                                setDeleting(spec);
                              },
                            },
                          ]
                        : []),
                    ]}
                  />
                </span>
              )}
            </li>
          ))}
        </ul>
      )}
      {failure && (
        <Callout tone="danger" role="alert">
          {failure}
        </Callout>
      )}
      {deleting && (
        <DeleteSpecDialog
          title={deleting.title}
          busy={false}
          onClose={() => {
            setDeleting(null);
          }}
          onConfirm={() => {
            const target = deleting;
            setDeleting(null);
            run(onDelete(target.id));
          }}
        />
      )}
    </section>
  );
}
