import { useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { Tabs } from '@/components/ui/Tabs';
import { Icon } from '@/components/ui/Icon';
import type { TranslationKey } from '@/i18n';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type {
  McpApplyReportDto,
  McpConfigDialectDto,
  McpConnectionDto,
  McpImportReportDto,
} from '@/lib/tauri/commands';
import {
  applyMcpToRuntime,
  exportMcpConfig,
  importMcpConfig,
  readRuntimeMcpConfig,
} from '../../services/mcpService';
import styles from './McpHub.module.css';

const DIALECTS: McpConfigDialectDto[] = [
  'claude',
  'gemini',
  'open_code',
  'codex',
  'copilot',
  'mcp',
];
const EXPORT_DIALECTS: McpConfigDialectDto[] = [...DIALECTS, 'antigravity'];
const EXTENSION: Record<McpConfigDialectDto, string> = {
  mcp: 'json',
  claude: 'json',
  gemini: 'json',
  open_code: 'json',
  codex: 'toml',
  copilot: 'json',
  antigravity: 'sh',
};

/** A reason code of a skipped server, in words. */
export function skipReason(t: (key: TranslationKey) => string, code: string): string {
  const key = `hub.skip.${code}` as TranslationKey;
  const text = t(key);
  return text === key ? t('hub.skip.other') : text;
}

function warningText(t: (key: TranslationKey) => string, code: string): string {
  const key = `hub.warn.${code}` as TranslationKey;
  const text = t(key);
  return text === key ? code : text;
}

export function ImportReport({ report }: { report: McpImportReportDto }) {
  const { t } = useI18n();
  return (
    <div className={styles.stack} role="status">
      <p className={styles.ok}>{t('hub.config.imported', { n: report.added.length })}</p>
      <ul className={styles.plainList}>
        {report.added.map((server) => (
          <li key={server.connectionId} className={styles.muted}>
            <strong>{server.name}</strong>
            {server.secretsStored.length > 0 &&
              ` · ${t('hub.config.secretsStored', { names: server.secretsStored.join(', ') })}`}
            {server.secretsMissing.length > 0 &&
              ` · ${t('hub.config.secretsMissing', { names: server.secretsMissing.join(', ') })}`}
            {server.credentialStoreFailed && ` · ${t('hub.config.storeFailed')}`}
            {server.warnings.map((w) => ` · ${warningText(t, w)}`).join('')}
          </li>
        ))}
      </ul>
      {report.skipped.length > 0 && (
        <>
          <p className={styles.callout}>{t('hub.config.skipped', { n: report.skipped.length })}</p>
          <ul className={styles.plainList}>
            {report.skipped.map((skipped) => (
              <li key={`${skipped.name}-${skipped.reason}`} className={styles.muted}>
                <strong>{skipped.name}</strong> — {skipReason(t, skipped.reason)}
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

interface Props {
  workspaceId: string;
  connections: McpConnectionDto[];
  /** A connection to pre-select for export. */
  selectedId: string | null;
  onChanged: () => void;
}

type Tab = 'import' | 'export';

/**
 * `atlas-mcp.json`: the same servers in the words of each CLI. Import reads a pasted text, a
 * file, or the CLI's own file; export writes the text, saves it, or adds it to the CLI's file
 * without overwriting anything.
 */
export function ConfigIo({ workspaceId, connections, selectedId, onChanged }: Props) {
  const { t } = useI18n();
  const [tab, setTab] = useState<Tab>('import');
  return (
    <div className={styles.stack}>
      <Tabs
        label={t('hub.config.title')}
        active={tab}
        onChange={setTab}
        items={(['import', 'export'] as const).map((name) => ({
          id: name,
          label: t(`hub.config.${name}`),
        }))}
      />
      {tab === 'import' ? (
        <ImportTab workspaceId={workspaceId} onChanged={onChanged} />
      ) : (
        <ExportTab workspaceId={workspaceId} connections={connections} selectedId={selectedId} />
      )}
    </div>
  );
}

function ImportTab({ workspaceId, onChanged }: { workspaceId: string; onChanged: () => void }) {
  const { t } = useI18n();
  const [dialect, setDialect] = useState<McpConfigDialectDto | 'auto'>('auto');
  const [text, setText] = useState('');
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [report, setReport] = useState<McpImportReportDto | null>(null);
  const [busy, setBusy] = useState(false);
  const file = useRef<HTMLInputElement>(null);

  function fromRuntime() {
    if (dialect === 'auto') return;
    setError(null);
    setReport(null);
    readRuntimeMcpConfig(workspaceId, dialect)
      .then((source) => {
        setText(source.text ?? '');
        setNote(
          source.text === null
            ? t('hub.config.noFile', { path: source.path })
            : t('hub.config.path', { path: source.path }),
        );
      })
      .catch((cause: unknown) => {
        setError(errorMessage(t, cause));
      });
  }

  function run() {
    setBusy(true);
    setError(null);
    importMcpConfig(workspaceId, text, dialect === 'auto' ? undefined : dialect)
      .then((result) => {
        setReport(result);
        onChanged();
      })
      .catch((cause: unknown) => {
        setError(errorMessage(t, cause));
      })
      .finally(() => {
        setBusy(false);
      });
  }

  return (
    <div className={styles.stack}>
      <p className={styles.muted}>{t('hub.config.importNote')}</p>
      <label className={styles.field}>
        {t('hub.config.dialect')}
        <select
          value={dialect}
          onChange={(e) => {
            setDialect(e.target.value as McpConfigDialectDto | 'auto');
          }}
        >
          <option value="auto">{t('hub.dialect.auto')}</option>
          {DIALECTS.map((d) => (
            <option key={d} value={d}>
              {t(`hub.dialect.${d}` as TranslationKey)}
            </option>
          ))}
        </select>
      </label>
      <div className={styles.row}>
        <Button
          variant="secondary"
          disabled={dialect === 'auto' || dialect === 'mcp'}
          title={dialect === 'auto' ? t('hub.config.pickFirst') : undefined}
          onClick={fromRuntime}
        >
          <Icon name="download" size={14} /> {t('hub.config.fromRuntime')}
        </Button>
        <Button
          variant="secondary"
          onClick={() => {
            file.current?.click();
          }}
        >
          <Icon name="upload" size={14} /> {t('hub.config.fromFile')}
        </Button>
        <input
          ref={file}
          type="file"
          hidden
          accept=".json,.toml,.jsonc,text/plain,application/json"
          aria-label={t('hub.config.fromFile')}
          onChange={(e) => {
            const picked = e.target.files?.[0];
            if (!picked) return;
            void picked.text().then((content) => {
              setText(content);
              setNote(picked.name);
              setReport(null);
            });
            e.target.value = '';
          }}
        />
      </div>
      {note && <p className={styles.muted}>{note}</p>}
      <label className={styles.field}>
        {t('hub.config.paste')}
        <textarea
          className={styles.code}
          rows={10}
          spellCheck={false}
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            setReport(null);
          }}
        />
      </label>
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      <div className={styles.row}>
        <Button disabled={busy || text.trim() === ''} onClick={run}>
          {t('hub.config.importAction')}
        </Button>
      </div>
      {report && <ImportReport report={report} />}
    </div>
  );
}

function ExportTab({
  workspaceId,
  connections,
  selectedId,
}: {
  workspaceId: string;
  connections: McpConnectionDto[];
  selectedId: string | null;
}) {
  const { t } = useI18n();
  const [dialect, setDialect] = useState<McpConfigDialectDto>('claude');
  const [onlySelected, setOnlySelected] = useState(selectedId !== null);
  const [text, setText] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [applied, setApplied] = useState<McpApplyReportDto | null>(null);
  const ids = onlySelected && selectedId ? [selectedId] : [];

  useEffect(() => {
    let cancelled = false;
    exportMcpConfig(workspaceId, dialect, onlySelected && selectedId ? [selectedId] : [])
      .then((result) => {
        if (!cancelled) {
          setText(result);
          setError(null);
        }
      })
      .catch((cause: unknown) => {
        if (!cancelled) setError(errorMessage(t, cause));
      });
    return () => {
      cancelled = true;
    };
  }, [workspaceId, dialect, onlySelected, selectedId, connections, t]);

  function save() {
    const url = URL.createObjectURL(new Blob([text], { type: 'text/plain' }));
    const link = document.createElement('a');
    link.href = url;
    link.download = `atlas-mcp.${EXTENSION[dialect]}`;
    link.click();
    URL.revokeObjectURL(url);
  }

  function copy() {
    void navigator.clipboard
      .writeText(text)
      .then(() => {
        setCopied(true);
        window.setTimeout(() => {
          setCopied(false);
        }, 1500);
      })
      .catch(() => {
        setError(t('hub.config.copyFailed'));
      });
  }

  return (
    <div className={styles.stack}>
      <p className={styles.muted}>{t('hub.config.exportNote')}</p>
      <label className={styles.field}>
        {t('hub.config.dialect')}
        <select
          value={dialect}
          onChange={(e) => {
            setDialect(e.target.value as McpConfigDialectDto);
            setApplied(null);
            setConfirming(false);
          }}
        >
          {EXPORT_DIALECTS.map((d) => (
            <option key={d} value={d}>
              {t(`hub.dialect.${d}` as TranslationKey)}
            </option>
          ))}
        </select>
      </label>
      {selectedId && (
        <label className={styles.check}>
          <input
            type="checkbox"
            checked={onlySelected}
            onChange={(e) => {
              setOnlySelected(e.target.checked);
            }}
          />
          {t('hub.config.onlySelected')}
        </label>
      )}
      {error && (
        <p role="alert" className={styles.error}>
          {error}
        </p>
      )}
      <textarea
        className={styles.code}
        rows={12}
        readOnly
        spellCheck={false}
        aria-label={t('hub.config.preview')}
        value={text}
      />
      {dialect === 'antigravity' && (
        <p className={styles.callout}>{t('hub.config.antigravityNote')}</p>
      )}
      <div className={styles.row}>
        <Button variant="secondary" disabled={text === ''} onClick={copy}>
          <Icon name="copy" size={14} /> {copied ? t('hub.config.copied') : t('hub.config.copy')}
        </Button>
        <Button variant="secondary" disabled={text === ''} onClick={save}>
          <Icon name="download" size={14} /> {t('hub.config.save')}
        </Button>
        {dialect !== 'antigravity' && (
          <Button
            variant="secondary"
            disabled={connections.length === 0}
            onClick={() => {
              setConfirming(true);
              setApplied(null);
            }}
          >
            {t('hub.config.apply')}
          </Button>
        )}
      </div>
      {confirming && (
        <div className={styles.callout} role="group" aria-label={t('hub.config.applyConfirm')}>
          <p>{t('hub.config.applyText')}</p>
          <div className={styles.row}>
            <Button
              onClick={() => {
                setConfirming(false);
                applyMcpToRuntime(workspaceId, dialect, ids)
                  .then(setApplied)
                  .catch((cause: unknown) => {
                    setError(errorMessage(t, cause));
                  });
              }}
            >
              {t('hub.config.applyConfirm')}
            </Button>
            <button
              type="button"
              className={styles.link}
              onClick={() => {
                setConfirming(false);
              }}
            >
              {t('common.cancel')}
            </button>
          </div>
        </div>
      )}
      {applied && (
        <p role="status" className={styles.ok}>
          {t('hub.config.applied', {
            path: applied.path,
            added: applied.added.length,
            kept: applied.kept.length,
          })}
          {applied.backup && ` ${t('hub.config.backup', { path: applied.backup })}`}
        </p>
      )}
    </div>
  );
}
