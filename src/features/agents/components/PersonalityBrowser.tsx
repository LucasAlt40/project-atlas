import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { useT } from '@/i18n/I18nProvider';
import type { CreatePersonalityInput, Personality } from '../types';
import { PersonalityCard } from './PersonalityCard';
import { PersonalityForm } from './PersonalityForm';
import styles from './Personality.module.css';

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

  return (
    <div>
      <div className={styles.toolbar}>
        <p>{t('personalities.intro')}</p>
        <div className={styles.actions}>
          <button
            type="button"
            className={styles.linkButton}
            onClick={() => {
              void onRestoreDefaults();
            }}
          >
            {t('personalities.restore')}
          </button>
          {!creating && (
            <Button
              onClick={() => {
                setEditingId(null);
                setCreating(true);
              }}
            >
              {t('personalities.new')}
            </Button>
          )}
        </div>
      </div>
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
      <div className={styles.list}>
        {personalities.map((personality) => (
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
    </div>
  );
}
