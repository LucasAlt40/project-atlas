import { useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { Icon } from '@/components/ui/Icon';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type {
  McpConfigDialectDto,
  McpPresetDto,
  McpRegistryServerDto,
  McpTransportDto,
} from '@/lib/tauri/commands';
import {
  addMcpConnection,
  applyMcpToRuntime,
  searchMcpRegistry,
  setMcpSecret,
} from '../../services/mcpService';
import styles from './McpHub.module.css';

type Tab = 'catalog' | 'registry' | 'custom';

interface Props {
  workspaceId: string;
  catalog: McpPresetDto[];
  /** Names already in the workspace. */
  existing: string[];
  onAddPreset: (presetId: string) => void;
  /** Called after something was added, so the hub reloads. */
  onAdded: () => void;
}

/** The three ways to bring a server in: what Atlas knows by name, the MCP Registry, or by hand. */
export function AddServer({ workspaceId, catalog, existing, onAddPreset, onAdded }: Props) {
  const { t } = useI18n();
  const [tab, setTab] = useState<Tab>('registry');
  return (
    <div className={styles.stack}>
      <div className={styles.tabsBar} role="tablist" aria-label={t('hub.add.title')}>
        {(['registry', 'catalog', 'custom'] as const).map((name) => (
          <button
            key={name}
            type="button"
            role="tab"
            aria-selected={tab === name}
            className={styles.tab}
            onClick={() => {
              setTab(name);
            }}
          >
            {t(`hub.add.${name}`)}
          </button>
        ))}
      </div>
      {tab === 'catalog' && (
        <ul className={styles.plainList}>
          {catalog.map((preset) => (
            <PresetRow
              key={preset.id}
              preset={preset}
              added={existing.includes(preset.connectionName)}
              onAdd={() => {
                onAddPreset(preset.id);
              }}
            />
          ))}
        </ul>
      )}
      {tab === 'registry' && (
        <RegistryTab workspaceId={workspaceId} existing={existing} onAdded={onAdded} />
      )}
      {tab === 'custom' && <CustomForm workspaceId={workspaceId} onAdded={onAdded} />}
    </div>
  );
}

function PresetRow({
  preset,
  added,
  onAdd,
}: {
  preset: McpPresetDto;
  added: boolean;
  onAdd: () => void;
}) {
  const { t } = useI18n();
  const planned = preset.availability !== 'ready';
  return (
    <li className={styles.preset} aria-label={preset.connectionName} data-planned={planned}>
      <div className={styles.rowBetween}>
        <span className={styles.presetTitle}>
          <strong>{t(`integrations.preset.${preset.id}.name` as TranslationKey)}</strong>
          <Badge mono tone="danger">
            {t('integrations.riskHigh')}
          </Badge>
        </span>
        <a href={preset.sourceUrl} target="_blank" rel="noreferrer" className={styles.link}>
          {t('integrations.source')} <Icon name="externalLink" size={12} />
        </a>
      </div>
      <p className={styles.muted}>
        {t(`integrations.preset.${preset.id}.about` as TranslationKey)}
      </p>
      {planned && <p className={styles.callout}>{t('integrations.needsHttp')}</p>}
      {preset.command && (
        <p className={styles.wouldRun}>
          <span>{t('integrations.wouldRun')}</span> <code>{preset.command}</code>
        </p>
      )}
      {preset.pinnedVersion && (
        <p className={styles.mutedItalic}>
          {t('integrations.pinned', { version: preset.pinnedVersion })}
        </p>
      )}
      {preset.requirements.some((r) => r.requirement === 'saved_sign_in' && !r.found) && (
        <p className={styles.callout}>
          {t('integrations.signInHint', { command: preset.command ?? '' })}
        </p>
      )}
      <div className={styles.presetFoot}>
        {preset.requirements.length > 0 && (
          <ul className={styles.requirements}>
            {preset.requirements.map((r) => (
              <li key={r.requirement}>
                <Badge tone={r.found ? 'success' : 'neutral'}>
                  {r.found ? '✓' : '·'} {t(`integrations.req.${r.requirement}` as TranslationKey)} —{' '}
                  {r.found ? t('integrations.reqFound') : t('integrations.reqNotFound')}
                </Badge>
              </li>
            ))}
          </ul>
        )}
        <Button disabled={planned || added} onClick={onAdd}>
          {added ? t('integrations.added') : t('integrations.add')}
        </Button>
      </div>
    </li>
  );
}

/** A server the user runs themselves: a program and its arguments, never a shell command line. */
function CustomForm({ workspaceId, onAdded }: { workspaceId: string; onAdded: () => void }) {
  const { t } = useI18n();
  const [name, setName] = useState('');
  const [executable, setExecutable] = useState('');
  const [args, setArgs] = useState('');
  const [plain, setPlain] = useState('');
  const [secrets, setSecrets] = useState('');
  const [error, setError] = useState<string | null>(null);

  const lines = (text: string) =>
    text
      .split('\n')
      .map((l) => l.trim())
      .filter(Boolean);
  const env: NonNullable<Extract<McpTransportDto, { kind: 'stdio' }>['env']> = [
    ...lines(plain).map((line) => {
      const at = line.indexOf('=');
      return {
        name: at < 0 ? line : line.slice(0, at),
        value: { kind: 'plain' as const, value: at < 0 ? '' : line.slice(at + 1) },
      };
    }),
    ...lines(secrets).map((secret) => ({ name: secret, value: { kind: 'secret' as const } })),
  ];

  return (
    <form
      className={styles.stack}
      aria-label={t('integrations.custom')}
      onSubmit={(e) => {
        e.preventDefault();
        setError(null);
        addMcpConnection(
          workspaceId,
          name.trim(),
          { kind: 'stdio', executable: executable.trim(), args: lines(args), env },
          false,
        )
          .then(() => {
            setName('');
            setExecutable('');
            setArgs('');
            setPlain('');
            setSecrets('');
            onAdded();
          })
          .catch((cause: unknown) => {
            setError(errorMessage(t, cause));
          });
      }}
    >
      <p className={styles.muted}>{t('integrations.customNote')}</p>
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      <label className={styles.field}>
        {t('integrations.customName')}
        <input
          value={name}
          onChange={(e) => {
            setName(e.target.value);
          }}
        />
      </label>
      <label className={styles.field}>
        {t('integrations.customExecutable')}
        <input
          value={executable}
          onChange={(e) => {
            setExecutable(e.target.value);
          }}
        />
      </label>
      <label className={styles.field}>
        {t('integrations.customArgs')}
        <textarea
          rows={3}
          value={args}
          onChange={(e) => {
            setArgs(e.target.value);
          }}
        />
      </label>
      <label className={styles.field}>
        {t('integrations.customEnv')}
        <textarea
          rows={2}
          value={plain}
          onChange={(e) => {
            setPlain(e.target.value);
          }}
        />
      </label>
      <label className={styles.field}>
        {t('integrations.customSecrets')}
        <textarea
          rows={2}
          value={secrets}
          onChange={(e) => {
            setSecrets(e.target.value);
          }}
        />
      </label>
      <div className={styles.row}>
        <Button type="submit" disabled={name.trim() === '' || executable.trim() === ''}>
          {t('integrations.customAdd')}
        </Button>
      </div>
    </form>
  );
}

/** The name an Atlas connection may have, from the registry's `io.github.owner/server`. */
export function connectionNameFor(registryName: string): string {
  const last = registryName.split('/').pop() ?? registryName;
  return (
    last
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, '-')
      .replace(/_{2,}/g, '_')
      .replace(/-{2,}/g, '-')
      .replace(/^[-_]+/, '')
      .slice(0, 32)
      .replace(/[-_]+$/, '') || 'server'
  );
}

const FILE_DIALECTS: McpConfigDialectDto[] = ['claude', 'gemini', 'open_code', 'codex', 'copilot'];

function RegistryTab({
  workspaceId,
  existing,
  onAdded,
}: {
  workspaceId: string;
  existing: string[];
  onAdded: () => void;
}) {
  const { t } = useI18n();
  const [query, setQuery] = useState('');
  const [state, setState] = useState<
    | { status: 'idle' }
    | { status: 'loading' }
    | { status: 'error'; message: string }
    | { status: 'ready'; servers: McpRegistryServerDto[]; next: string | null }
  >({ status: 'idle' });
  const [open, setOpen] = useState<string | null>(null);

  function search(cursor?: string) {
    setState((previous) =>
      cursor !== undefined && previous.status === 'ready' ? previous : { status: 'loading' },
    );
    searchMcpRegistry(query.trim(), cursor)
      .then((page) => {
        setState((previous) => ({
          status: 'ready',
          servers: [
            ...(cursor !== undefined && previous.status === 'ready' ? previous.servers : []),
            ...page.servers,
          ],
          next: page.nextCursor,
        }));
      })
      .catch((cause: unknown) => {
        setState({ status: 'error', message: errorMessage(t, cause) });
      });
  }

  return (
    <div className={styles.stack}>
      <p className={styles.muted}>{t('hub.registry.note')}</p>
      <form
        className={styles.row}
        role="search"
        onSubmit={(e) => {
          e.preventDefault();
          search();
        }}
      >
        <label className={styles.search}>
          <Icon name="search" size={14} />
          <input
            aria-label={t('hub.registry.search')}
            placeholder={t('hub.registry.search')}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
            }}
          />
        </label>
        <Button type="submit" variant="secondary" disabled={state.status === 'loading'}>
          {t('hub.registry.searchAction')}
        </Button>
      </form>
      {state.status === 'idle' && <p className={styles.muted}>{t('hub.registry.idle')}</p>}
      {state.status === 'loading' && (
        <p className={styles.muted} role="status">
          {t('hub.registry.loading')}
        </p>
      )}
      {state.status === 'error' && (
        <p role="alert" className={styles.error}>
          {state.message}
        </p>
      )}
      {state.status === 'ready' && state.servers.length === 0 && (
        <p className={styles.muted}>{t('hub.registry.empty')}</p>
      )}
      {state.status === 'ready' && (
        <ul className={styles.plainList}>
          {state.servers.map((server) => (
            <RegistryRow
              key={`${server.name}@${server.version}`}
              server={server}
              expanded={open === server.name}
              taken={existing.includes(connectionNameFor(server.name))}
              workspaceId={workspaceId}
              onToggle={() => {
                setOpen(open === server.name ? null : server.name);
              }}
              onAdded={onAdded}
            />
          ))}
        </ul>
      )}
      {state.status === 'ready' && state.next !== null && (
        <div className={styles.row}>
          <Button
            variant="secondary"
            onClick={() => {
              if (state.next) search(state.next);
            }}
          >
            {t('hub.registry.more')}
          </Button>
        </div>
      )}
    </div>
  );
}

function RegistryRow({
  server,
  expanded,
  taken,
  workspaceId,
  onToggle,
  onAdded,
}: {
  server: McpRegistryServerDto;
  expanded: boolean;
  taken: boolean;
  workspaceId: string;
  onToggle: () => void;
  onAdded: () => void;
}) {
  const { t } = useI18n();
  const installable = server.installs.length > 0;
  return (
    <li className={styles.sub} aria-label={server.name}>
      <button
        type="button"
        className={styles.rowButton}
        aria-expanded={expanded}
        onClick={onToggle}
      >
        <span className={styles.rowBetween}>
          <strong>{server.title ?? server.name}</strong>
          <span className={styles.chips}>
            <span className={styles.chip}>v{server.version}</span>
            {installable ? (
              <span className={styles.chip} data-tone="ok">
                {server.installs.map((i) => i.kind).join(' · ')}
              </span>
            ) : (
              <span className={styles.chip} data-tone="warn">
                {server.remotes.length > 0
                  ? t('hub.registry.remoteOnly')
                  : t('hub.registry.noInstall')}
              </span>
            )}
          </span>
        </span>
        <span className={styles.muted}>{server.description}</span>
        <code className={styles.mono}>{server.name}</code>
      </button>
      {expanded && (
        <RegistryDetail server={server} taken={taken} workspaceId={workspaceId} onAdded={onAdded} />
      )}
    </li>
  );
}

function RegistryDetail({
  server,
  taken,
  workspaceId,
  onAdded,
}: {
  server: McpRegistryServerDto;
  taken: boolean;
  workspaceId: string;
  onAdded: () => void;
}) {
  const { t } = useI18n();
  const [which, setWhich] = useState(0);
  const [name, setName] = useState(connectionNameFor(server.name));
  const [values, setValues] = useState<Record<string, string>>({});
  const [alsoWrite, setAlsoWrite] = useState<ReadonlySet<McpConfigDialectDto>>(new Set());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);
  const install = server.installs[which];

  const setValue = (key: string, value: string) => {
    setValues((current) => ({ ...current, [key]: value }));
  };

  // What the person would see run: the program and what follows it.
  const argv = install
    ? [
        install.executable,
        ...install.args,
        ...install.inputs.flatMap((input, index) => {
          const value = values['arg:' + String(index)] ?? input.value ?? '';
          if (value === '' && !input.flag) return [];
          return [...(input.flag ? [input.flag] : []), ...(value === '' ? [] : [value])];
        }),
      ]
    : [];

  function add() {
    if (!install) return;
    type EnvEntry = NonNullable<Extract<McpTransportDto, { kind: 'stdio' }>['env']>[number];
    const env = install.env.flatMap((spec): EnvEntry[] => {
      if (spec.secret) return [{ name: spec.name, value: { kind: 'secret' as const } }];
      const value = values[`env:${spec.name}`] ?? spec.default ?? '';
      if (value === '' && !spec.required) return [];
      return [{ name: spec.name, value: { kind: 'plain' as const, value } }];
    });
    setBusy(true);
    setError(null);
    addMcpConnection(
      workspaceId,
      name.trim(),
      { kind: 'stdio', executable: argv[0] ?? '', args: argv.slice(1), env },
      false,
    )
      .then(async (connection) => {
        for (const spec of install.env) {
          const secret = values[`env:${spec.name}`];
          if (spec.secret && secret) await setMcpSecret(connection.id, spec.name, secret);
        }
        for (const dialect of alsoWrite) {
          await applyMcpToRuntime(workspaceId, dialect, [connection.id]);
        }
        setDone(connection.name);
        onAdded();
      })
      .catch((cause: unknown) => {
        setError(errorMessage(t, cause));
      })
      .finally(() => {
        setBusy(false);
      });
  }

  const missingRequired =
    !install ||
    install.inputs.some(
      (input, i) => input.required && !(values['arg:' + String(i)] ?? input.value),
    ) ||
    install.env.some(
      (spec) => spec.required && !spec.secret && !(values[`env:${spec.name}`] ?? spec.default),
    );

  return (
    <div className={styles.stack}>
      {server.repositoryUrl && (
        <a href={server.repositoryUrl} target="_blank" rel="noreferrer" className={styles.link}>
          {t('integrations.source')}
        </a>
      )}
      {!install && (
        <p className={styles.callout}>
          {server.remotes.length > 0
            ? t('hub.registry.remoteExplain', { url: server.remotes[0] ?? '' })
            : t('hub.registry.noInstallExplain', { kinds: server.unsupportedPackages.join(', ') })}
        </p>
      )}
      {install && (
        <>
          {server.installs.length > 1 && (
            <label className={styles.field}>
              {t('hub.registry.installWith')}
              <select
                value={which}
                onChange={(e) => {
                  setWhich(Number(e.target.value));
                }}
              >
                {server.installs.map((option, index) => (
                  <option key={`${option.kind}-${option.executable}`} value={index}>
                    {option.kind} ({option.executable})
                  </option>
                ))}
              </select>
            </label>
          )}
          <p className={styles.muted}>{t('hub.registry.howTo')}</p>
          <code className={styles.pre}>{argv.join(' ')}</code>
          {['npx', 'uvx'].includes(install.executable) && (
            <p className={styles.muted}>{t('integrations.examineDownloads')}</p>
          )}
          <label className={styles.field}>
            {t('integrations.customName')}
            <input
              value={name}
              onChange={(e) => {
                setName(e.target.value);
              }}
            />
          </label>
          {install.inputs.map((input, index) =>
            input.value === null || input.required ? (
              <label
                key={`${input.flag ?? 'arg'}-${input.description ?? String(index)}`}
                className={styles.field}
              >
                {input.flag ?? t('hub.registry.argument')}
                {input.required ? ' *' : ''}
                {input.description ? ` — ${input.description}` : ''}
                <input
                  value={values['arg:' + String(index)] ?? input.value ?? ''}
                  onChange={(e) => {
                    setValue('arg:' + String(index), e.target.value);
                  }}
                />
              </label>
            ) : null,
          )}
          {install.env.map((spec) => (
            <label key={spec.name} className={styles.field}>
              <code className={styles.mono}>{spec.name}</code>
              {spec.required ? ' *' : ''}
              {spec.secret ? ` (${t('hub.registry.secret')})` : ''}
              {spec.description ? ` — ${spec.description}` : ''}
              <input
                type={spec.secret ? 'password' : 'text'}
                autoComplete="off"
                placeholder={spec.default ?? ''}
                value={values[`env:${spec.name}`] ?? ''}
                onChange={(e) => {
                  setValue(`env:${spec.name}`, e.target.value);
                }}
              />
            </label>
          ))}
          <fieldset className={styles.fieldset}>
            <legend>{t('hub.registry.alsoWrite')}</legend>
            {FILE_DIALECTS.map((dialect) => (
              <label key={dialect} className={styles.check}>
                <input
                  type="checkbox"
                  checked={alsoWrite.has(dialect)}
                  onChange={(e) => {
                    setAlsoWrite((current) => {
                      const next = new Set(current);
                      if (e.target.checked) next.add(dialect);
                      else next.delete(dialect);
                      return next;
                    });
                  }}
                />
                {t(`hub.dialect.${dialect}` as TranslationKey)}
              </label>
            ))}
          </fieldset>
          {error && (
            <p role="alert" className={styles.error}>
              {error}
            </p>
          )}
          {done ? (
            <p role="status" className={styles.ok}>
              {t('hub.registry.added', { name: done })}
            </p>
          ) : (
            <div className={styles.row}>
              <Button
                disabled={busy || taken || missingRequired || name.trim() === ''}
                onClick={add}
              >
                {taken ? t('integrations.added') : t('integrations.add')}
              </Button>
            </div>
          )}
        </>
      )}
    </div>
  );
}
