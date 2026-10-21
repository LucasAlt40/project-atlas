import { useState, type ReactNode } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { OverflowMenu, type OverflowItem } from '@/components/ui/OverflowMenu';
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
  const rejected = memories.filter((m) => m.status === 'rejected');
  const kept = memories.filter((m) => m.status !== 'candidate' && m.status !== 'rejected');
  const automatic = memories.filter((m) => m.provenance.automatic).length;
  // What waits for the person comes first when there is anything; else the settings.
  // The section that opens is decided once, when the notes arrive; after that only the person moves.
  const [chosen, setChosen] = useState<Tab | null>(null);
  const [firstTab, setFirstTab] = useState<Tab | null>(null);
  if (firstTab === null && state.status === 'ready') {
    setFirstTab(pending.length > 0 ? 'pending' : 'usage');
  }
  const picked: Tab = chosen ?? firstTab ?? 'usage';
  // The last turned-down note was approved or forgotten: its tab is gone, so is the person.
  const tab: Tab = picked === 'rejected' && rejected.length === 0 ? 'kept' : picked;
  const tabs: { id: Tab; label: string }[] = [
    { id: 'usage', label: t('memory.modes.title') },
    { id: 'pending', label: `${t('memory.pending.title')} (${String(pending.length)})` },
    { id: 'kept', label: `${t('memory.notes.title')} (${String(kept.length)})` },
    ...(rejected.length > 0
      ? [
          {
            id: 'rejected' as const,
            label: `${t('memory.rejected.title')} (${String(rejected.length)})`,
          },
        ]
      : []),
    { id: 'add', label: t('memory.form.add') },
  ];

  return (
    <div className={styles.memory}>
      <p className={styles.lead}>{t('memory.title')}</p>
      <p className={styles.help}>{t('memory.help')}</p>

      <div className={styles.tabs} role="tablist" aria-label={t('memory.tabs')}>
        {tabs.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            className={styles.tab}
            aria-selected={tab === item.id}
            onClick={() => {
              setChosen(item.id);
            }}
          >
            {item.label}
          </button>
        ))}
      </div>

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

      {tab === 'usage' && (
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
                  const picked = modesOf(scope).find((m) => m === event.target.value);
                  if (picked)
                    void attempt(() => changeMemorySettings({ ...memory, [scope]: picked }));
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
          <MaxTokens memory={memory} onSave={(next) => attempt(() => changeMemorySettings(next))} />
          <p className={styles.help}>{t('memory.modes.help')}</p>
        </fieldset>
      )}

      {tab === 'pending' && (
        <section aria-label={t('memory.pending.title')}>
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
                      onClick={() => void attempt(() => actions.reject(note.id))}
                    >
                      {t('memory.reject')}
                    </Button>
                  </>
                }
              />
            ))}
          </ul>
        </section>
      )}

      {tab === 'kept' && (
        <section aria-label={t('memory.notes.title')}>
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
                    {(note.status === 'archived' || note.status === 'stale') && (
                      <Button
                        variant="secondary"
                        onClick={() => void attempt(() => actions.setStatus(note.id, 'active'))}
                      >
                        {t('memory.restore')}
                      </Button>
                    )}
                  </>
                }
                menu={[
                  ...(note.status === 'archived' || note.status === 'stale'
                    ? []
                    : [
                        {
                          label: t('memory.archive'),
                          icon: 'folder' as const,
                          onSelect: () => {
                            void attempt(() => actions.setStatus(note.id, 'archived'));
                          },
                        },
                      ]),
                  {
                    label: t('memory.delete'),
                    icon: 'trash' as const,
                    danger: true,
                    onSelect: () => {
                      void attempt(() => actions.remove(note.id));
                    },
                  },
                ]}
              />
            ))}
          </ul>
          {automatic > 0 && (
            <Button variant="danger" onClick={() => void attempt(actions.clearAutomatic)}>
              {t('memory.deleteAutomatic', { count: automatic })}
            </Button>
          )}
        </section>
      )}

      {tab === 'rejected' && (
        <section aria-label={t('memory.rejected.title')}>
          <p className={styles.help}>{t('memory.rejected.help')}</p>
          <ul className={styles.list}>
            {rejected.map((note) => (
              <NoteCard
                key={note.id}
                note={note}
                onEdit={(title, content) =>
                  attempt(() => actions.edit(note.id, title, content, note.topic ?? null))
                }
                actions={
                  <Button
                    variant="secondary"
                    onClick={() => void attempt(() => actions.approve(note.id))}
                  >
                    {t('memory.approveAnyway')}
                  </Button>
                }
                menu={[
                  {
                    label: t('memory.forget'),
                    icon: 'trash' as const,
                    danger: true,
                    onSelect: () => {
                      void attempt(() => actions.remove(note.id));
                    },
                  },
                ]}
              />
            ))}
          </ul>
        </section>
      )}

      {tab === 'add' && <AddNote onAdd={(input) => attempt(() => actions.create(input))} />}
    </div>
  );
}

type Tab = 'usage' | 'pending' | 'kept' | 'rejected' | 'add';

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
  menu = [],
  onEdit,
}: {
  note: MemoryDto;
  actions: ReactNode;
  /** The rarer actions: archive and delete. */
  menu?: OverflowItem[];
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

  const badges: {
    key: string;
    tone: 'warning' | 'neutral' | 'success';
    short: string;
    full: string;
  }[] = [];
  if (note.provenance.automatic)
    badges.push({
      key: 'auto',
      tone: 'warning',
      short: t('memory.unconfirmed'),
      full: t('memory.automatic'),
    });
  if (note.provenance.automatic && note.status === 'candidate')
    badges.push({
      key: 'held',
      tone: 'warning',
      short: t('memory.heldShort'),
      full: t('memory.heldForValidator'),
    });
  if (note.provenance.validatedBy)
    badges.push({
      key: 'validated',
      tone: 'success',
      short: t('memory.validated'),
      full: t('memory.validated'),
    });
  if (
    note.provenance.origin === 'agent' &&
    note.status === 'candidate' &&
    !note.provenance.automatic
  )
    badges.push({
      key: 'proposed',
      tone: 'neutral',
      short: t('memory.proposedByAgent'),
      full: t('memory.proposedByAgent'),
    });
  // Put away because the runs it was in kept being thrown away (ADR 0041), not because a file left.
  const misled = note.misled ?? 0;
  const helped = note.helped ?? 0;
  if (
    note.status === 'stale' &&
    note.provenance.origin !== 'user' &&
    misled >= 2 &&
    misled > helped
  )
    badges.push({
      key: 'misled',
      tone: 'warning',
      short: t('memory.misledShort'),
      full: t('memory.misled', { count: misled }),
    });
  else if (note.status === 'stale' || note.status === 'archived')
    badges.push({
      key: 'status',
      tone: 'warning',
      short: t(`memory.status.${note.status}` as TranslationKey),
      full: t(`memory.status.${note.status}` as TranslationKey),
    });

  return (
    <li className={styles.note} data-status={note.status}>
      <div className={styles.noteHead}>
        <div className={styles.noteTitle}>
          <strong>{note.title}</strong>
          {badges.map((badge) => (
            <Badge key={badge.key} dot mono caps={false} tone={badge.tone} title={badge.full}>
              {badge.short === badge.full ? (
                badge.full
              ) : (
                <>
                  <span aria-hidden="true">{badge.short}</span>
                  <span className={styles.srOnly}>{badge.full}</span>
                </>
              )}
            </Badge>
          ))}
        </div>
        <span className={styles.uses}>
          {t('memory.uses', { count: note.uses })}
          {helped + misled > 0 ? ` · ${t('memory.verdicts', { helped, misled })}` : ''}
        </span>
      </div>
      <span className={styles.meta}>
        {t(`memory.kind.${note.kind}` as TranslationKey)} · {scope}
        {ownerName ? ` · ${ownerName}` : ''}
      </span>
      <p className={styles.content}>{note.content}</p>
      <div className={styles.noteFoot}>
        {actions}
        <Button
          variant="ghost"
          onClick={() => {
            setEditing(true);
          }}
        >
          {t('memory.edit')}
        </Button>
        {menu.length > 0 && (
          <OverflowMenu label={t('memory.moreActions', { name: note.title })} items={menu} />
        )}
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
