import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import { useT } from '@/i18n/I18nProvider';
import type { CreatePersonalityInput, Personality } from '../types';
import { PersonalityCard } from './PersonalityCard';
import { PersonalityForm } from './PersonalityForm';
import styles from './Personality.module.css';

type Filter = 'all' | 'builtin' | 'custom';

interface Props {
  personalities: Personality[];
  onUse: (personalityId: string) => void;
  onCreate: (input: CreatePersonalityInput) => Promise<Personality>;
  onEdit: (id: string, input: CreatePersonalityInput) => Promise<Personality>;
  onDelete: (id: string) => Promise<void>;
  onRestoreDefaults: () => Promise<void>;
}

export function PersonalityBrowser({
  personalities,
  onUse,
  onCreate,
  onEdit,
  onDelete,
  onRestoreDefaults,
}: Props) {
  const t = useT();
  const [creating, setCreating] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const editing = personalities.find((p) => p.id === editingId);

  const [filter, setFilter] = useState<Filter>('all');
  const [query, setQuery] = useState('');
  const count = (f: Filter) =>
    personalities.filter(
      (p) => f === 'all' || p.source === (f === 'builtin' ? 'builtin' : 'custom'),
    ).length;
  const needle = query.trim().toLowerCase();
  const visible = personalities.filter((p) => {
    if (filter !== 'all' && p.source !== (filter === 'builtin' ? 'builtin' : 'custom'))
      return false;
    if (!needle) return true;
    return `${p.name} ${p.description} ${p.systemInstructions} ${p.tags.join(' ')}`
      .toLowerCase()
      .includes(needle);
  });
  const panelOpen = creating || editing !== undefined;

  return (
    <div className={styles.browser}>
      <div className={styles.toolbar}>
        <p>{t('personalities.intro')}</p>
        <div className={styles.actions}>
          <button
            type="button"
            className={styles.secondaryButton}
            onClick={() => {
              void onRestoreDefaults();
            }}
          >
            <Icon name="refresh" size={14} />
            {t('personalities.restore')}
          </button>
          {!creating && (
            <Button
              onClick={() => {
                setEditingId(null);
                setCreating(true);
              }}
            >
              <Icon name="plus" size={16} />
              {t('personalities.new')}
            </Button>
          )}
        </div>
      </div>

      <div className={styles.filterBar}>
        <div className={styles.tabsBar} role="group" aria-label={t('personalities.title')}>
          {(['all', 'builtin', 'custom'] as const).map((name) => (
            <button
              key={name}
              type="button"
              className={styles.filterTab}
              aria-pressed={filter === name}
              onClick={() => {
                setFilter(name);
              }}
            >
              {t(`personalities.filter.${name}`)} ({count(name)})
            </button>
          ))}
        </div>
        <label className={styles.searchBox}>
          <Icon name="search" size={16} />
          <input
            type="search"
            value={query}
            placeholder={t('personalities.search')}
            aria-label={t('personalities.search')}
            onChange={(e) => {
              setQuery(e.target.value);
            }}
          />
        </label>
      </div>

      <div className={styles.layout} data-panel={panelOpen}>
        <div className={styles.list}>
          {visible.length === 0 && <p className={styles.note}>{t('personalities.noMatch')}</p>}
          {visible.map((personality) => (
            <PersonalityCard
              key={personality.id}
              personality={personality}
              onUse={onUse}
              onEdit={(id) => {
                setCreating(false);
                setEditingId(id);
              }}
              onDelete={onDelete}
            />
          ))}
        </div>
        {panelOpen && (
          <aside className={styles.sidePanel}>
            <h2 className={styles.panelHeading}>
              {editing ? t('personalities.panel.edit') : t('personalities.form.new')}
              <span className={styles.badge}>
                {t(
                  editing?.source === 'builtin'
                    ? 'personalities.source.builtin'
                    : 'personalities.source.custom',
                )}
              </span>
            </h2>
            {creating && (
              <PersonalityForm
                onSubmit={onCreate}
                onSaved={() => {
                  setCreating(false);
                }}
                onCancel={() => {
                  setCreating(false);
                }}
              />
            )}
            {editing && (
              <PersonalityForm
                key={editing.id}
                initial={editing}
                onSubmit={(input) => onEdit(editing.id, input)}
                onSaved={() => {
                  setEditingId(null);
                }}
                onCancel={() => {
                  setEditingId(null);
                }}
              />
            )}
          </aside>
        )}
      </div>
    </div>
  );
}
