import { useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Callout } from '@/components/ui/Callout';
import { Drawer } from '@/components/ui/Drawer';
import { Icon } from '@/components/ui/Icon';
import { OverflowMenu } from '@/components/ui/OverflowMenu';
import { PageHeader } from '@/components/ui/PageHeader';
import { SegmentedControl } from '@/components/ui/SegmentedControl';
import { useBreadcrumb } from '@/app/BreadcrumbContext';
import { ImportDialog } from '@/features/portability';
import { useT } from '@/i18n/I18nProvider';
import type { CreatePersonalityInput, Personality } from '../types';
import { PersonalityCard } from './PersonalityCard';
import { PersonalityForm } from './PersonalityForm';
import styles from './Personality.module.css';

type Filter = 'all' | 'builtin' | 'custom';

const FORM_ID = 'personality-form';

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
  const [importing, setImporting] = useState(false);
  const [protocolOpen, setProtocolOpen] = useState(false);
  /** One card is open at a time; the others stay compact. */
  const [openId, setOpenId] = useState<string | null>(null);
  const editing = personalities.find((p) => p.id === editingId);
  useBreadcrumb([creating && t('personalities.form.new'), editing?.name]);

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
  const custom = count('custom');
  const panelTitle = editing ? t('personalities.panel.edit') : t('personalities.form.new');

  return (
    <div className={styles.browser}>
      <PageHeader
        title={t('personalities.title')}
        meta={`${t('personalities.builtinCount', { n: count('builtin') })} · ${t(
          custom === 1 ? 'personalities.customCount.one' : 'personalities.customCount',
          { n: custom },
        )}`}
        actions={
          <>
            <Button
              onClick={() => {
                setEditingId(null);
                setCreating(true);
              }}
            >
              <Icon name="plus" size={16} />
              {t('personalities.new')}
            </Button>
            <OverflowMenu
              label={t('personalities.more')}
              items={[
                {
                  label: t('portability.import'),
                  icon: 'download',
                  onSelect: () => {
                    setImporting(true);
                  },
                },
                {
                  label: t('personalities.restore'),
                  icon: 'refresh',
                  onSelect: () => {
                    void onRestoreDefaults();
                  },
                },
              ]}
            />
          </>
        }
      />

      <Callout
        tone="info"
        action={
          <Button
            variant="ghost"
            size="sm"
            aria-expanded={protocolOpen}
            onClick={() => {
              setProtocolOpen((open) => !open);
            }}
          >
            {t(protocolOpen ? 'common.seeLess' : 'common.seeMore')}
          </Button>
        }
      >
        {t('personalities.protocol.summary')}
        {protocolOpen && <span className={styles.protocolText}>{t('personalities.note')}</span>}
      </Callout>

      <div className={styles.controls}>
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
        <SegmentedControl
          label={t('personalities.filter.label')}
          value={filter}
          onChange={setFilter}
          segments={(['all', 'builtin', 'custom'] as const).map((name) => ({
            value: name,
            label: `${t(`personalities.filter.${name}`)} (${String(count(name))})`,
          }))}
        />
      </div>

      {visible.length === 0 && <p className={styles.note}>{t('personalities.noMatch')}</p>}
      <div className={styles.list}>
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
        <Drawer
          label={panelTitle}
          onClose={closePanel}
          width="lg"
          title={
            <>
              {panelTitle}
              <Badge>
                {t(
                  editing?.source === 'builtin'
                    ? 'personalities.source.builtin'
                    : 'personalities.source.custom',
                )}
              </Badge>
            </>
          }
          footer={
            <>
              <Button type="button" variant="secondary" onClick={closePanel}>
                {t('common.cancel')}
              </Button>
              <Button type="submit" form={FORM_ID}>
                {t('personalities.form.save')}
              </Button>
            </>
          }
        >
          {creating && <PersonalityForm id={FORM_ID} onSubmit={onCreate} onSaved={closePanel} />}
          {editing && (
            <PersonalityForm
              id={FORM_ID}
              key={editing.id}
              initial={editing}
              onSubmit={(input) => onEdit(editing.id, input)}
              onSaved={closePanel}
            />
          )}
        </Drawer>
      )}

      {importing && (
        <ImportDialog
          onImported={onImported}
          onClose={() => {
            setImporting(false);
          }}
        />
      )}
    </div>
  );
}
