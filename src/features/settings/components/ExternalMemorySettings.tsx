import { useEffect, useId, useRef, useState } from 'react';
import { Button } from '@/components/ui/Button';
import { useI18n } from '@/i18n/I18nProvider';
import { errorMessage } from '@/i18n/messages';
import type { ExternalMemorySettingsDto } from '@/lib/tauri/commands';
import {
  checkExternalMemory,
  getSettings,
  setExternalMemory,
  syncExternalMemory,
} from '../services/settingsService';
import styles from '../pages/SettingsPage.module.css';

const OFF: ExternalMemorySettingsDto = {
  enabled: false,
  serverUrl: 'http://127.0.0.1:49374',
  dataDir: '',
  maxHits: 3,
  maxTokens: 400,
  workspace: 'default',
  share: false,
  precise: true,
};

const REASONS = [
  'address',
  'data_dir',
  'token',
  'not_running',
  'timeout',
  'refused',
  'unreadable',
  'too_large',
  'server',
  'only_atlas_pages',
  'sharing_off',
] as const;

/** The words for a reason code from the core; an unknown code is said plainly, never hidden. */
function reasonText(
  t: ReturnType<typeof useI18n>['t'],
  code: string | null,
  detail: string | null,
): string {
  const known = REASONS.find((r) => r === code);
  // `server` is "answered with status N": without the N there is nothing to say
  if (!known || (known === 'server' && !detail)) return t('settings.externalMemory.reason.unknown');
  return t(`settings.externalMemory.reason.${known}`, { detail: detail ?? '' });
}

/**
 * A memory server on this computer (ADR 0040): Atlas reads notes from it before a run and, when the
 * person allows, shares the notes they approve. Off until switched on; the address must be a
 * loopback one, and the access token is read from the server's own folder, never kept here.
 *
 * The three text fields are saved together by their own button; each switch saves only itself, so a
 * half-typed address is never saved by flipping a switch. Nothing can be pressed while a request is
 * in flight, so two answers can never arrive out of order.
 */
export function ExternalMemorySettings() {
  const { t } = useI18n();
  // the language can change while this is open; the words are looked up when needed, and the
  // settings are not reloaded (that would throw away what the person typed)
  const words = useRef(t);
  useEffect(() => {
    words.current = t;
  }, [t]);
  const ids = {
    url: useId(),
    dataDir: useId(),
    workspace: useId(),
    precise: useId(),
    share: useId(),
  };
  const [settings, setSettings] = useState<ExternalMemorySettingsDto | null>(null);
  const [url, setUrl] = useState('');
  const [dataDir, setDataDir] = useState('');
  const [workspace, setWorkspace] = useState('');
  const [failure, setFailure] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  function showFields(current: ExternalMemorySettingsDto) {
    setUrl(current.serverUrl);
    setDataDir(current.dataDir);
    setWorkspace(current.workspace);
  }

  useEffect(() => {
    let cancelled = false;
    Promise.resolve(getSettings())
      .then((loaded) => {
        if (cancelled) return;
        const current = loaded.externalMemory ?? OFF;
        setSettings(current);
        showFields(current);
      })
      .catch((error: unknown) => {
        if (!cancelled) setFailure(errorMessage(words.current, error));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  /** One request at a time; the answer to the last one is the one shown. */
  async function run(work: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setFailure(null);
    setOutcome(null);
    try {
      await work();
    } catch (error) {
      setFailure(errorMessage(words.current, error));
    } finally {
      setBusy(false);
    }
  }

  /** Saves `next`; `withFields` says whether the text fields are part of it and take the saved values. */
  function save(next: ExternalMemorySettingsDto, withFields: boolean) {
    return run(async () => {
      const saved = (await setExternalMemory(next)).externalMemory ?? next;
      setSettings(saved);
      if (withFields) showFields(saved);
    });
  }

  function check() {
    return run(async () => {
      const result = await checkExternalMemory();
      setOutcome(
        result.reachable
          ? words.current('settings.externalMemory.checkOk')
          : words.current('settings.externalMemory.checkFail', {
              reason: reasonText(words.current, result.reason, result.detail),
            }),
      );
    });
  }

  function sync() {
    return run(async () => {
      const result = await syncExternalMemory();
      setOutcome(
        result.ok
          ? words.current('settings.externalMemory.syncDone', {
              published: String(result.published),
              retired: String(result.retired),
              failed: String(result.failed),
            })
          : words.current('settings.externalMemory.syncFail', {
              reason: reasonText(words.current, result.reason, result.detail),
            }),
      );
    });
  }

  if (!settings) return failure ? <p role="alert">{failure}</p> : null;

  const edited =
    url !== settings.serverUrl || dataDir !== settings.dataDir || workspace !== settings.workspace;
  const typed = { ...settings, serverUrl: url, dataDir, workspace };

  return (
    <div className={styles.stack}>
      <p className={styles.help}>{t('settings.externalMemory.help')}</p>
      <div className={styles.group}>
        <label className={styles.toggle}>
          <input
            type="checkbox"
            checked={settings.enabled}
            disabled={busy}
            onChange={(event) => {
              void save({ ...settings, enabled: event.target.checked }, false);
            }}
          />
          {t('settings.externalMemory.enabled')}
        </label>
      </div>
      <div className={styles.group}>
        <label htmlFor={ids.url}>{t('settings.externalMemory.url')}</label>
        <input
          id={ids.url}
          type="text"
          value={url}
          spellCheck={false}
          disabled={busy}
          aria-describedby={`${ids.url}-help`}
          onChange={(event) => {
            setUrl(event.target.value);
            setOutcome(null);
          }}
        />
        <p id={`${ids.url}-help`} className={styles.help}>
          {t('settings.externalMemory.urlHelp')}
        </p>
      </div>
      <div className={styles.group}>
        <label htmlFor={ids.dataDir}>{t('settings.externalMemory.dataDir')}</label>
        <input
          id={ids.dataDir}
          type="text"
          value={dataDir}
          spellCheck={false}
          disabled={busy}
          aria-describedby={`${ids.dataDir}-help`}
          onChange={(event) => {
            setDataDir(event.target.value);
            setOutcome(null);
          }}
        />
        <p id={`${ids.dataDir}-help`} className={styles.help}>
          {t('settings.externalMemory.dataDirHelp')}
        </p>
      </div>
      <div className={styles.group}>
        <label htmlFor={ids.workspace}>{t('settings.externalMemory.workspace')}</label>
        <input
          id={ids.workspace}
          type="text"
          value={workspace}
          spellCheck={false}
          disabled={busy}
          aria-describedby={`${ids.workspace}-help`}
          onChange={(event) => {
            setWorkspace(event.target.value);
            setOutcome(null);
          }}
        />
        <p id={`${ids.workspace}-help`} className={styles.help}>
          {t('settings.externalMemory.workspaceHelp')}
        </p>
      </div>
      <div className={styles.group}>
        <label className={styles.toggle}>
          <input
            type="checkbox"
            checked={settings.share}
            disabled={busy || !settings.enabled}
            aria-describedby={`${ids.share}-help`}
            onChange={(event) => {
              void save({ ...settings, share: event.target.checked }, false);
            }}
          />
          {t('settings.externalMemory.share')}
        </label>
        <p id={`${ids.share}-help`} className={styles.help}>
          {t('settings.externalMemory.shareHelp')}
          {!settings.enabled && ` ${t('settings.externalMemory.needsEnabled')}`}
        </p>
      </div>
      <div className={styles.group}>
        <label className={styles.toggle}>
          <input
            type="checkbox"
            checked={settings.precise}
            disabled={busy}
            aria-describedby={`${ids.precise}-help`}
            onChange={(event) => {
              void save({ ...settings, precise: event.target.checked }, false);
            }}
          />
          {t('settings.externalMemory.precise')}
        </label>
        <p id={`${ids.precise}-help`} className={styles.help}>
          {t('settings.externalMemory.preciseHelp')}
        </p>
      </div>
      <div className={styles.actions}>
        <Button
          disabled={busy || !edited}
          onClick={() => {
            void save(typed, true);
          }}
        >
          {t('settings.externalMemory.save')}
        </Button>
        <Button
          disabled={busy || !settings.enabled || edited}
          onClick={() => {
            void check();
          }}
        >
          {t('settings.externalMemory.check')}
        </Button>
        <Button
          disabled={busy || !settings.enabled || !settings.share || edited}
          onClick={() => {
            void sync();
          }}
        >
          {t('settings.externalMemory.sync')}
        </Button>
      </div>
      {outcome && <p role="status">{outcome}</p>}
      {failure && <p role="alert">{failure}</p>}
    </div>
  );
}
