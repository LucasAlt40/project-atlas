import { useMemo, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useNow } from '@/features/agents/hooks/useNow';
import type { Agent } from '@/features/agents/types';
import { useI18n } from '@/i18n/I18nProvider';
import { countsByGroup, GROUPS, inGroup, type Group } from '../model/status';
import type { SpecSummary } from '../types';
import { SpecCard } from './SpecCard';
import styles from './Specs.module.css';

interface Props {
  specs: SpecSummary[];
  agents: Agent[];
  onOpen: (specId: string) => void;
  onNew: () => void;
  onImport: () => void;
}

/** The workspace's specs: what is going on, what needs the person, what is ready to build. */
export function Overview({ specs, agents, onOpen, onNew, onImport }: Props) {
  const { t } = useI18n();
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
      <div className={styles.filters}>
        <div className={styles.tabs} role="group" aria-label={t('specs.filter')}>
          {GROUPS.map((name) => (
            <button
              key={name}
              type="button"
              className={styles.tab}
              aria-pressed={group === name}
              data-group={name}
              onClick={() => {
                setGroup(name);
              }}
            >
              {t(`specs.group.${name}`)}
              <span className={styles.chipCount}>{counts[name]}</span>
            </button>
          ))}
        </div>
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
      </div>
      {shown.length === 0 ? (
        <p className={styles.muted} role="status">
          {t('specs.noMatches')}
        </p>
      ) : (
        <ul className={styles.grid}>
          {shown.map((spec) => (
            <li key={spec.id}>
              <SpecCard
                spec={spec}
                now={now}
                agentName={agents.find((a) => a.id === spec.authorAgentId)?.name ?? '—'}
                onOpen={onOpen}
              />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
