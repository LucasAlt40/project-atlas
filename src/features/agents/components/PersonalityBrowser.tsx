import { useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { CollapsibleSection } from '@/components/ui/CollapsibleSection';
import { Icon } from '@/components/ui/Icon';
import { Modal } from '@/components/ui/Modal';
import { ImportButton } from '@/features/portability';
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
  /** An import changed the catalogue. */
  onImported: () => void;
}

export function PersonalityBrowser({
  personalities,
  onUse,
  onCreate,
  onEdit,
  onDelete,
  onRestoreDefaults,
  onImported,
}: Props) {
  const t = useT();
  const [creating, setCreating] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  /** One card is open at a time; the others stay compact. */
  const [openId, setOpenId] = useState<string | null>(null);
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
  const closePanel = () => {
    setCreating(false);
    setEditingId(null);
  };

  return (
    <div className={styles.browser}>
      <header className={styles.pageHead}>
        <div className={styles.headText}>
          <h1 className={styles.title}>{t('personalities.title')}</h1>
          <div className={styles.chips}>
            <Badge mono>{t('personalities.available', { n: personalities.length })}</Badge>
            <Badge mono>{t('personalities.customCount', { n: count('custom') })}</Badge>
            <Badge mono>{t('personalities.builtinCount', { n: count('builtin') })}</Badge>
          </div>
          <p className={styles.intro}>{t('personalities.intro')}</p>
        </div>
        <div className={styles.actions}>
          <ImportButton onImported={onImported} />
          <Button
            variant="ghost"
            onClick={() => {
              void onRestoreDefaults();
            }}
          >
            <Icon name="refresh" size={14} />
            {t('personalities.restore')}
          </Button>
          <Button
            onClick={() => {
              setEditingId(null);
              setCreating(true);
            }}
          >
            <Icon name="plus" size={16} />
            {t('personalities.new')}
          </Button>
        </div>
      </header>

      <CollapsibleSection title={t('personalities.protocol.title')} defaultOpen={false}>
        <p className={styles.protocolText}>{t('personalities.note')}</p>
      </CollapsibleSection>

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

      <div className={styles.list}>
        {visible.length === 0 && <p className={styles.note}>{t('personalities.noMatch')}</p>}
        {visible.map((personality) => (
          <PersonalityCard
            key={personality.id}
            personality={personality}
            expanded={openId === personality.id}
            onToggle={() => {
              setOpenId((current) => (current === personality.id ? null : personality.id));
            }}
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
        <Modal
          label={editing ? t('personalities.panel.edit') : t('personalities.form.new')}
          onClose={closePanel}
          placement="side"
          size="md"
        >
          <h2 className={styles.panelHeading}>
            {editing ? t('personalities.panel.edit') : t('personalities.form.new')}
            <Badge mono dot tone={editing?.source === 'builtin' ? 'info' : 'policy'}>
              {t(
                editing?.source === 'builtin'
                  ? 'personalities.source.builtin'
                  : 'personalities.source.custom',
              )}
            </Badge>
          </h2>
          {creating && (
            <PersonalityForm panel onSubmit={onCreate} onSaved={closePanel} onCancel={closePanel} />
          )}
          {editing && (
            <PersonalityForm
              panel
              key={editing.id}
              initial={editing}
              onSubmit={(input) => onEdit(editing.id, input)}
              onSaved={closePanel}
              onCancel={closePanel}
            />
          )}
        </Modal>
      )}
    </div>
  );
}
