import { useState } from 'react';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { CollapsibleSection } from '@/components/ui/CollapsibleSection';
import { OverflowMenu } from '@/components/ui/OverflowMenu';
import type { Agent, RuntimeStatus } from '@/features/agents/types';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import type {
  McpConnectionDto,
  McpGrantDto,
  McpTransportDto,
  WorkflowDto,
} from '@/lib/tauri/commands';
import { envOf, manifestOf, statusOf } from '../../model/hub';
import {
  clearMcpSecret,
  probeMcpConnection,
  removeMcpConnection,
  setMcpConnectionEnabled,
  setMcpSecret,
  updateMcpConnection,
} from '../../services/mcpService';
import { ExamineRow, FETCHES_CODE, SecretRow } from '../connectionParts';
import { GrantAgents } from './GrantAgents';
import { StatusPill } from './ServerCard';
import integrations from '../Integrations.module.css';
import styles from './McpHub.module.css';

type Tab = 'tools' | 'env' | 'manifest' | 'activity';

interface Props {
  connection: McpConnectionDto;
  grants: McpGrantDto[];
  agents: Agent[];
  runtimes: RuntimeStatus[];
  workflows: WorkflowDto[];
  riskNoteKey: TranslationKey | undefined;
  confirmingOn: boolean;
  busy: boolean;
  run: (action: () => Promise<unknown>) => void;
  onConfirmOn: (confirming: boolean) => void;
  onExport: () => void;
}

/** The inspector of one server: what it offers, who may use it, its variables, how it starts. */
export function Inspector({
  connection,
  grants,
  agents,
  runtimes,
  workflows,
  riskNoteKey,
  confirmingOn,
  busy,
  run,
  onConfirmOn,
  onExport,
}: Props) {
  const { t, language } = useI18n();
  const [tab, setTab] = useState<Tab>('tools');
  const [removing, setRemoving] = useState(false);
  const tools = connection.discovery?.tools ?? [];
  const usable = runtimes.filter(
    (r) =>
      r.runtime.capabilities.mcp === 'supported' &&
      r.runtime.capabilities.mcpFeatures.probe !== 'none',
  );
  const stdio = connection.transport.kind === 'stdio' ? connection.transport : undefined;
  const program =
    stdio?.executable
      .split(/[\\/]/)
      .pop()
      ?.replace(/\.\w+$/, '') ?? '';
  const env = envOf(connection);
  const secretNames = env.filter((e) => e.value.kind === 'secret').map((e) => e.name);
  const missing = secretNames.filter((name) => !(name in connection.secrets));
  const mine = grants.filter((g) => g.connectionId === connection.id);

  return (
    <aside className={styles.inspector} aria-label={t('hub.inspector.title')}>
      <header className={styles.inspectorHead}>
        <div>
          <span className={styles.eyebrow}>{t('hub.inspector.title')}</span>
          <h2 className={styles.inspectorName}>{connection.name}</h2>
        </div>
        <StatusPill status={statusOf(connection)} />
      </header>

      {confirmingOn && (
        <div className={styles.callout} role="group" aria-label={t('integrations.confirmOn')}>
          <p>{t('integrations.confirmOnText')}</p>
          {riskNoteKey && <p>{t(riskNoteKey)}</p>}
          <div className={styles.row}>
            <Button
              disabled={busy}
              onClick={() => {
                onConfirmOn(false);
                run(() => setMcpConnectionEnabled(connection.id, true));
              }}
            >
              {t('integrations.confirmOn')}
            </Button>
            <button
              type="button"
              className={styles.link}
              onClick={() => {
                onConfirmOn(false);
              }}
            >
              {t('common.cancel')}
            </button>
          </div>
        </div>
      )}

      <div className={styles.tabsBar} role="tablist" aria-label={t('hub.inspector.title')}>
        {(['tools', 'env', 'manifest', 'activity'] as const).map((name) => (
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
            {t(`hub.tab.${name}` as TranslationKey, { n: tools.length })}
          </button>
        ))}
      </div>

      <div className={styles.inspectorBody} role="tabpanel">
        {tab === 'tools' && (
          <>
            <section className={styles.stack}>
              <h3 className={integrations.heading}>{t('integrations.examine')}</h3>
              <p className={styles.muted}>
                {t('integrations.examineNote')}
                {FETCHES_CODE.has(program) && ` ${t('integrations.examineDownloads')}`}
              </p>
              <ExamineRow
                connection={connection}
                runtimes={usable}
                disabled={busy || !connection.enabled || missing.length > 0}
                onExamine={(runtimeId) => {
                  run(() => probeMcpConnection(connection.id, runtimeId));
                }}
              />
              {tools.length > 0 && (
                <ul className={styles.toolList} aria-label={t('hub.tools.list')}>
                  {tools.map((tool) => (
                    <li key={tool}>
                      <code className={styles.mono}>{tool}</code>
                    </li>
                  ))}
                </ul>
              )}
              <p className={styles.muted}>{t('hub.tools.noRpc')}</p>
            </section>

            <section className={styles.stack}>
              <h3 className={integrations.heading}>{t('integrations.grants')}</h3>
              <p className={styles.muted}>{t('integrations.grantsNote')}</p>
              <GrantAgents
                key={mine.map((g) => g.id).join(',')}
                connection={connection}
                grants={grants}
                agents={agents}
                runtimes={runtimes}
                workflows={workflows}
                busy={busy}
                run={run}
              />
            </section>
          </>
        )}

        {tab === 'env' && (
          <section className={styles.stack}>
            {env.length === 0 && <p className={styles.muted}>{t('hub.env.none')}</p>}
            {env
              .filter((e) => e.value.kind === 'plain')
              .map((e) => (
                <div key={e.name} className={styles.rowBetween}>
                  <code className={styles.mono}>{e.name}</code>
                  <code className={styles.mono}>
                    {e.value.kind === 'plain' ? e.value.value : ''}
                  </code>
                </div>
              ))}
            {secretNames.length > 0 && (
              <>
                <h3 className={integrations.heading}>{t('integrations.secrets')}</h3>
                <p className={styles.muted}>{t('integrations.secretsNote')}</p>
                {secretNames.map((name) => (
                  <SecretRow
                    key={name}
                    name={name}
                    storedAt={connection.secrets[name]}
                    busy={busy}
                    onSave={(value) => {
                      run(() => setMcpSecret(connection.id, name, value));
                    }}
                    onClear={() => {
                      run(() => clearMcpSecret(connection.id, name));
                    }}
                  />
                ))}
              </>
            )}
          </section>
        )}

        {tab === 'manifest' && (
          <ManifestEditor
            key={connection.id + JSON.stringify(connection.transport)}
            connection={connection}
            busy={busy}
            run={run}
          />
        )}

        {tab === 'activity' && (
          <section className={styles.stack}>
            {connection.discovery ? (
              <dl className={styles.facts}>
                <dt>{t('hub.activity.status')}</dt>
                <dd>{t(`hub.status.${connection.discovery.status}` as TranslationKey)}</dd>
                <dt>{t('hub.activity.runtime')}</dt>
                <dd>{connection.discovery.runtimeId}</dd>
                <dt>{t('hub.activity.when')}</dt>
                <dd>{new Date(connection.discovery.discoveredAt).toLocaleString(language)}</dd>
                <dt>{t('hub.activity.tools')}</dt>
                <dd>{connection.discovery.tools.length}</dd>
              </dl>
            ) : (
              <p className={styles.muted}>{t('hub.activity.none')}</p>
            )}
            <p className={styles.muted}>{t('hub.activity.noLogs')}</p>
          </section>
        )}

        <CollapsibleSection
          title={t('hub.guardrails.title')}
          defaultOpen={false}
          aside={
            <Badge mono caps={false}>
              {t('hub.guardrails.badge')}
            </Badge>
          }
        >
          <ul className={styles.plainList}>
            {(['untrusted', 'unauthorized', 'secrets'] as const).map((rule) => (
              <li key={rule} className={styles.muted}>
                ✓ {t(`hub.guardrails.${rule}` as TranslationKey)}
              </li>
            ))}
          </ul>
        </CollapsibleSection>
      </div>

      <footer className={styles.inspectorFoot}>
        {removing ? (
          <div className={styles.row} role="group" aria-label={t('integrations.removeConfirm')}>
            <Button
              variant="danger"
              disabled={busy}
              onClick={() => {
                run(() => removeMcpConnection(connection.id));
              }}
            >
              {t('integrations.removeConfirm')}
            </Button>
            <Button
              variant="ghost"
              onClick={() => {
                setRemoving(false);
              }}
            >
              {t('common.cancel')}
            </Button>
          </div>
        ) : (
          <OverflowMenu
            label={t('hub.inspector.more', { name: connection.name })}
            items={[
              { label: t('hub.export'), icon: 'upload', onSelect: onExport },
              {
                label: t('hub.remove'),
                icon: 'trash',
                danger: true,
                onSelect: () => {
                  setRemoving(true);
                },
              },
            ]}
          />
        )}
      </footer>
    </aside>
  );
}

/** How Atlas starts the server, as JSON; edited as the transport itself and saved switched off. */
function ManifestEditor({
  connection,
  busy,
  run,
}: {
  connection: McpConnectionDto;
  busy: boolean;
  run: (action: () => Promise<unknown>) => void;
}) {
  const { t } = useI18n();
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(JSON.stringify(connection.transport, null, 2));
  let parsed: McpTransportDto | null = null;
  try {
    const value: unknown = JSON.parse(text);
    if (value && typeof value === 'object' && 'kind' in value) parsed = value as McpTransportDto;
  } catch {
    parsed = null;
  }
  return (
    <section className={styles.stack}>
      <p className={styles.muted}>{t('hub.manifest.note')}</p>
      {editing ? (
        <>
          <textarea
            className={styles.code}
            rows={12}
            spellCheck={false}
            aria-label={t('hub.manifest.edit')}
            value={text}
            onChange={(e) => {
              setText(e.target.value);
            }}
          />
          {parsed === null && (
            <p role="alert" className={styles.error}>
              {t('hub.manifest.invalid')}
            </p>
          )}
          <p className={styles.callout}>{t('hub.manifest.saveNote')}</p>
          <div className={styles.row}>
            <Button
              disabled={busy || parsed === null}
              onClick={() => {
                if (parsed)
                  run(() => updateMcpConnection(connection.id, parsed, connection.required));
              }}
            >
              {t('hub.manifest.save')}
            </Button>
            <button
              type="button"
              className={styles.link}
              onClick={() => {
                setEditing(false);
                setText(JSON.stringify(connection.transport, null, 2));
              }}
            >
              {t('common.cancel')}
            </button>
          </div>
        </>
      ) : (
        <>
          <pre className={styles.pre}>{manifestOf(connection)}</pre>
          <div className={styles.row}>
            <Button
              variant="secondary"
              onClick={() => {
                setEditing(true);
              }}
            >
              {t('hub.manifest.edit')}
            </Button>
          </div>
        </>
      )}
    </section>
  );
}
