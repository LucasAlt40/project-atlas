import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { useCatalog } from '@/features/agents/hooks/useCatalog';
import { useSettings } from '@/features/settings/hooks/SettingsProvider';
import { useOptionalWorkspace } from '@/features/workspace/hooks/WorkspaceProvider';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type {
  MemoryDto,
  MemoryKindDto,
  MemoryModeDto,
  MemoryScopeDto,
  MemorySettingsDto,
} from '@/lib/tauri/commands';
import { useMemories } from '../hooks/useMemories';
import styles from './Memory.module.css';

const MAX_CONTENT = 480;
const MAX_TITLE = 80;
const KINDS: readonly MemoryKindDto[] = ['how_to', 'pitfall', 'preference', 'decision'];
const SCOPES: readonly MemoryScopeDto[] = ['user', 'workspace', 'agent'];

/** The user's own scope takes no proposals from an agent: it speaks for the person. */
const modesOf = (scope: MemoryScopeDto): readonly MemoryModeDto[] =>
  scope === 'user' ? ['off', 'propose'] : ['off', 'propose', 'auto'];

/**
 * Notes of earlier work (ADR 0037): which scopes Atlas recalls and lets agents add to, what waits
 * for a decision, and what is kept. A note is a hint a model may use; it never grants anything.
 */
export function MemorySettings() {
  const { t } = useI18n();
  const { memory, changeMemorySettings } = useSettings();
  const { state, ...actions } = useMemories();
  const [failure, setFailure] = useState<string | null>(null);

  async function attempt(work: () => Promise<unknown>) {
    setFailure(null);
    try {
      await work();
    } catch (error) {
      setFailure(errorMessage(t, error));
    }
  }

  const memories = state.status === 'ready' ? state.memories : [];
  const pending = memories.filter((m) => m.status === 'candidate');
  const kept = memories.filter((m) => m.status !== 'candidate');
  const automatic = memories.filter((m) => m.provenance.automatic).length;

  return (
    <div className={styles.memory}>
      <p className={styles.help}>{t('memory.help')}</p>

      <fieldset className={styles.modes}>
        <legend className={styles.label}>{t('memory.modes.title')}</legend>
        {SCOPES.map((scope) => (
          <div key={scope} className={styles.mode}>
            <label htmlFor={`memory-mode-${scope}`}>
              {t(`memory.scope.${scope}` as TranslationKey)}
            </label>
            <select
              id={`memory-mode-${scope}`}
              className={styles.control}
              value={memory[scope]}
              onChange={(event) => {
                const chosen = modesOf(scope).find((m) => m === event.target.value);
                if (chosen)
                  void attempt(() => changeMemorySettings({ ...memory, [scope]: chosen }));
              }}
            >
              {modesOf(scope).map((mode) => (
                <option key={mode} value={mode}>
                  {t(`memory.mode.${mode}` as TranslationKey)}
                </option>
              ))}
            </select>
          </div>
        ))}
        <p className={styles.help}>{t('memory.modes.help')}</p>
        <MaxTokens memory={memory} onSave={(next) => attempt(() => changeMemorySettings(next))} />
      </fieldset>

      {failure && (
        <p role="alert" className={styles.error}>
          {failure}
        </p>
      )}
      {state.status === 'error' && (
        <p role="alert" className={styles.error}>
          {errorMessage(t, state.error)}
        </p>
      )}

      {pending.length > 0 && (
        <section aria-label={t('memory.pending.title')}>
          <h3 className={styles.heading}>{t('memory.pending.title')}</h3>
          <p className={styles.help}>{t('memory.pending.help')}</p>
          <ul className={styles.list}>
            {pending.map((note) => (
              <NoteCard
                key={note.id}
                note={note}
                onEdit={(title, content) =>
                  attempt(() => actions.edit(note.id, title, content, note.topic ?? null))
                }
                actions={
                  <>
                    <Button
                      variant="primary"
                      onClick={() => void attempt(() => actions.approve(note.id))}
                    >
                      {t('memory.approve')}
                    </Button>
                    <Button
                      variant="ghost"
                      onClick={() => void attempt(() => actions.remove(note.id))}
                    >
                      {t('memory.discard')}
                    </Button>
                  </>
                }
              />
            ))}
          </ul>
        </section>
      )}

      <section aria-label={t('memory.notes.title')}>
        <h3 className={styles.heading}>{t('memory.notes.title')}</h3>
        {kept.length === 0 && <p className={styles.help}>{t('memory.notes.empty')}</p>}
        <ul className={styles.list}>
          {kept.map((note) => (
            <NoteCard
              key={note.id}
              note={note}
              onEdit={(title, content) =>
                attempt(() => actions.edit(note.id, title, content, note.topic ?? null))
              }
              actions={
                <>
                  {note.provenance.automatic && (
                    <Button
                      variant="secondary"
                      onClick={() => void attempt(() => actions.promote(note.id))}
                    >
                      {t('memory.promote')}
                    </Button>
                  )}
                  {note.status === 'archived' || note.status === 'stale' ? (
                    <Button
                      variant="secondary"
                      onClick={() => void attempt(() => actions.setStatus(note.id, 'active'))}
                    >
                      {t('memory.restore')}
                    </Button>
                  ) : (
                    <Button
                      variant="ghost"
                      onClick={() => void attempt(() => actions.setStatus(note.id, 'archived'))}
                    >
                      {t('memory.archive')}
                    </Button>
                  )}
                  <Button
                    variant="danger"
                    onClick={() => void attempt(() => actions.remove(note.id))}
                  >
                    {t('memory.delete')}
                  </Button>
                </>
              }
            />
          ))}
        </ul>
        {automatic > 0 && (
          <Button variant="ghost" onClick={() => void attempt(actions.clearAutomatic)}>
            {t('memory.deleteAutomatic', { count: automatic })}
          </Button>
        )}
      </section>

      <AddNote onAdd={(input) => attempt(() => actions.create(input))} />
    </div>
  );
}

function MaxTokens({
  memory,
  onSave,
}: {
  memory: MemorySettingsDto;
  onSave: (next: MemorySettingsDto) => Promise<void>;
}) {
  const { t } = useI18n();
  const [value, setValue] = useState(String(memory.maxTokens));
  return (
    <form
      className={styles.inline}
      onSubmit={(event) => {
        event.preventDefault();
        const parsed = Math.floor(Number(value.replace(/[.,\s]/g, '')));
        if (parsed >= 0) void onSave({ ...memory, maxTokens: parsed });
      }}
    >
      <label htmlFor="memory-max-tokens">{t('memory.maxTokens')}</label>
      <input
        id="memory-max-tokens"
        className={styles.control}
        inputMode="numeric"
        value={value}
        onChange={(event) => {
          setValue(event.target.value);
        }}
      />
      <Button type="submit" variant="secondary">
        {t('memory.saveLimit')}
      </Button>
    </form>
  );
}

function NoteCard({
  note,
  actions,
  onEdit,
}: {
  note: MemoryDto;
  actions: React.ReactNode;
  onEdit: (title: string, content: string) => Promise<void>;
}) {
  const { t } = useI18n();
  const workspace = useOptionalWorkspace();
  const { catalog } = useCatalog();
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(note.title);
  const [content, setContent] = useState(note.content);

  const ownerName =
    note.scope === 'workspace'
      ? workspace?.workspaces.find((w) => w.id === note.owner)?.name
      : note.scope === 'agent' && catalog.status === 'ready'
        ? catalog.agents.find((a) => a.id === note.owner)?.name
        : undefined;
  const scope = t(`memory.scope.${note.scope}` as TranslationKey);

  if (editing) {
    return (
      <li className={styles.note}>
        <input
          aria-label={t('memory.form.title')}
          className={styles.control}
          maxLength={MAX_TITLE}
          value={title}
          onChange={(event) => {
            setTitle(event.target.value);
          }}
        />
        <textarea
          aria-label={t('memory.form.content')}
          className={styles.area}
          maxLength={MAX_CONTENT}
          value={content}
          onChange={(event) => {
            setContent(event.target.value);
          }}
        />
        <div className={styles.row}>
          <Button
            variant="primary"
            disabled={title.trim() === '' || content.trim() === ''}
            onClick={() => {
              void onEdit(title, content).then(() => {
                setEditing(false);
              });
            }}
          >
            {t('memory.save')}
          </Button>
          <Button
            variant="ghost"
            onClick={() => {
              setEditing(false);
            }}
          >
            {t('memory.cancel')}
          </Button>
        </div>
      </li>
    );
  }

  return (
    <li className={styles.note} data-status={note.status}>
      <div className={styles.noteHead}>
        <strong>{note.title}</strong>
        <span className={styles.meta}>
          {t(`memory.kind.${note.kind}` as TranslationKey)} · {scope}
          {ownerName ? ` · ${ownerName}` : ''}
        </span>
        {note.provenance.automatic && <span className={styles.badge}>{t('memory.automatic')}</span>}
        {note.provenance.origin === 'agent' && note.status === 'candidate' && (
          <span className={styles.badge}>{t('memory.proposedByAgent')}</span>
        )}
        {(note.status === 'stale' || note.status === 'archived') && (
          <span className={styles.badge} data-tone="warning">
            {t(`memory.status.${note.status}` as TranslationKey)}
          </span>
        )}
      </div>
      <p className={styles.content}>{note.content}</p>
      <div className={styles.row}>
        <span className={styles.meta}>{t('memory.uses', { count: note.uses })}</span>
        {actions}
        <Button
          variant="ghost"
          onClick={() => {
            setEditing(true);
          }}
        >
          {t('memory.edit')}
        </Button>
      </div>
    </li>
  );
}

function AddNote({
  onAdd,
}: {
  onAdd: (input: {
    scope: MemoryScopeDto;
    owner: string | null;
    kind: MemoryKindDto;
    title: string;
    content: string;
  }) => Promise<void>;
}) {
  const { t } = useI18n();
  const workspace = useOptionalWorkspace();
  const { catalog } = useCatalog();
  const [scope, setScope] = useState<MemoryScopeDto>('user');
  const [kind, setKind] = useState<MemoryKindDto>('how_to');
  const [owner, setOwner] = useState('');
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');

  const owners =
    scope === 'workspace'
      ? (workspace?.workspaces ?? []).map((w) => ({ id: w.id, name: w.name }))
      : scope === 'agent' && catalog.status === 'ready'
        ? catalog.agents.map((a) => ({ id: a.id, name: a.name }))
        : [];
  const ownerId = scope === 'user' ? null : ((owner || owners[0]?.id) ?? null);
  const ready =
    title.trim() !== '' && content.trim() !== '' && (scope === 'user' || ownerId !== null);

  return (
    <form
      className={styles.add}
      aria-label={t('memory.form.add')}
      onSubmit={(event) => {
        event.preventDefault();
        if (!ready) return;
        void onAdd({ scope, owner: ownerId, kind, title, content }).then(() => {
          setTitle('');
          setContent('');
        });
      }}
    >
      <h3 className={styles.heading}>{t('memory.form.add')}</h3>
      <div className={styles.row}>
        <select
          aria-label={t('memory.form.scope')}
          className={styles.control}
          value={scope}
          onChange={(event) => {
            const chosen = SCOPES.find((s) => s === event.target.value);
            if (chosen) {
              setScope(chosen);
              setOwner('');
            }
          }}
        >
          {SCOPES.map((s) => (
            <option key={s} value={s}>
              {t(`memory.scope.${s}` as TranslationKey)}
            </option>
          ))}
        </select>
        {scope !== 'user' && (
          <select
            aria-label={t('memory.form.owner')}
            className={styles.control}
            value={ownerId ?? ''}
            onChange={(event) => {
              setOwner(event.target.value);
            }}
          >
            {owners.map((o) => (
              <option key={o.id} value={o.id}>
                {o.name}
              </option>
            ))}
          </select>
        )}
        <select
          aria-label={t('memory.form.kind')}
          className={styles.control}
          value={kind}
          onChange={(event) => {
            const chosen = KINDS.find((k) => k === event.target.value);
            if (chosen) setKind(chosen);
          }}
        >
          {KINDS.map((k) => (
            <option key={k} value={k}>
              {t(`memory.kind.${k}` as TranslationKey)}
            </option>
          ))}
        </select>
      </div>
      <input
        aria-label={t('memory.form.title')}
        className={styles.control}
        maxLength={MAX_TITLE}
        value={title}
        onChange={(event) => {
          setTitle(event.target.value);
        }}
      />
      <textarea
        aria-label={t('memory.form.content')}
        className={styles.area}
        maxLength={MAX_CONTENT}
        value={content}
        onChange={(event) => {
          setContent(event.target.value);
        }}
      />
      <p className={styles.help}>
        {t('memory.form.limit', { used: content.length, max: MAX_CONTENT })}
      </p>
      <Button type="submit" disabled={!ready}>
        {t('memory.form.submit')}
      </Button>
    </form>
  );
}
