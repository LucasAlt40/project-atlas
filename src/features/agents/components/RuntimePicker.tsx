import { useState } from 'react';
import type { TranslationKey } from '@/i18n';
import { useT } from '@/i18n/I18nProvider';
import type { RuntimeStatus } from '../types';
import styles from './Form.module.css';

interface Props {
  runtimes: RuntimeStatus[];
  runtimeId: string;
  modelId: string;
  onRuntimeChange: (runtimeId: string) => void;
  onModelChange: (modelId: string) => void;
}

/** Hint codes the core may send for the model field, and their text. */
const MODEL_HINTS: Record<string, TranslationKey> = {
  claude_alias: 'runtime.hint.claude_alias',
  gemini_model: 'runtime.hint.gemini_model',
};

type UnselectableReason = 'notInstalled' | 'unavailable' | 'notSupported';

/** Why a runtime cannot be chosen, or `null` if it can. */
export function unselectableReason(status: RuntimeStatus): UnselectableReason | null {
  if (status.availability === 'not_installed') return 'notInstalled';
  if (status.availability === 'unavailable') return 'unavailable';
  if (!status.runtime.capabilities.nonInteractiveExecution) return 'notSupported';
  return null;
}

interface ProviderGroup {
  id: string;
  name: string;
  runtimes: RuntimeStatus[];
}

function groupByProvider(runtimes: RuntimeStatus[]): ProviderGroup[] {
  const groups: ProviderGroup[] = [];
  for (const status of runtimes) {
    const { provider } = status.runtime;
    const group = groups.find((g) => g.id === provider.id);
    if (group) group.runtimes.push(status);
    else groups.push({ id: provider.id, name: provider.name, runtimes: [status] });
  }
  return groups;
}

function ModelField({
  status,
  modelId,
  onModelChange,
}: {
  status: RuntimeStatus;
  modelId: string;
  onModelChange: (modelId: string) => void;
}) {
  const t = useT();
  const { name, modelHint } = status.runtime;
  const hasList = status.modelDiscovery === 'discovered' && status.availableModels.length > 0;
  const hintKey = modelHint ? MODEL_HINTS[modelHint] : undefined;
  return (
    <div className={styles.field}>
      <label htmlFor="model-input" className={styles.label}>
        {t('runtime.model')}
      </label>
      {hasList ? (
        <select
          id="model-input"
          className={styles.control}
          value={modelId}
          onChange={(e) => {
            onModelChange(e.target.value);
          }}
        >
          <option value="">{t('runtime.selectModel')}</option>
          {status.availableModels.map((model) => (
            <option key={model.id} value={model.id}>
              {model.name}
            </option>
          ))}
        </select>
      ) : (
        <>
          <input
            id="model-input"
            className={styles.control}
            value={modelId}
            placeholder={t('runtime.modelId')}
            onChange={(e) => {
              onModelChange(e.target.value);
            }}
          />
          <p className={styles.hint}>
            {status.modelDiscovery === 'failed'
              ? t('runtime.discovery.failed', { runtime: name })
              : status.modelDiscovery === 'discovered'
                ? t('runtime.discovery.empty', { runtime: name })
                : t('runtime.discovery.unsupported', { runtime: name })}{' '}
            {t('runtime.enterModel')} {hintKey ? t(hintKey) : ''}
          </p>
        </>
      )}
    </div>
  );
}

/**
 * Provider (who) → Connection (which runtime on this machine) → Model. What is shown for
 * each step comes from the runtime's own status and capabilities; nothing here knows any
 * particular provider.
 */
export function RuntimePicker({
  runtimes,
  runtimeId,
  modelId,
  onRuntimeChange,
  onModelChange,
}: Props) {
  const t = useT();
  const groups = groupByProvider(runtimes);
  const selected = runtimes.find((r) => r.runtime.id === runtimeId);
  const [providerId, setProviderId] = useState(selected?.runtime.provider.id ?? '');
  const group = groups.find((g) => g.id === providerId);
  const reasonText = (reason: UnselectableReason) => t(`runtime.reason.${reason}`);

  function chooseProvider(id: string) {
    setProviderId(id);
    const first = groups
      .find((g) => g.id === id)
      ?.runtimes.find((r) => unselectableReason(r) === null);
    onRuntimeChange(first?.runtime.id ?? '');
  }

  return (
    <>
      <div className={styles.field}>
        <label htmlFor="provider-select" className={styles.label}>
          {t('runtime.provider')}
        </label>
        <select
          id="provider-select"
          className={styles.control}
          value={providerId}
          onChange={(e) => {
            chooseProvider(e.target.value);
          }}
        >
          <option value="">{t('runtime.selectProvider')}</option>
          {groups.map((g) => {
            const usable = g.runtimes.some((r) => unselectableReason(r) === null);
            const first = g.runtimes[0];
            const reason = first ? unselectableReason(first) : null;
            return (
              <option key={g.id} value={g.id} disabled={!usable}>
                {usable ? g.name : `${g.name} (${reasonText(reason ?? 'unavailable')})`}
              </option>
            );
          })}
        </select>
      </div>

      {group && (
        <div className={styles.field}>
          <label htmlFor="runtime-select" className={styles.label}>
            {t('runtime.connection')}
          </label>
          <select
            id="runtime-select"
            className={styles.control}
            value={runtimeId}
            onChange={(e) => {
              onRuntimeChange(e.target.value);
            }}
          >
            <option value="">{t('runtime.selectConnection')}</option>
            {group.runtimes.map((status) => {
              const reason = unselectableReason(status);
              return (
                <option
                  key={status.runtime.id}
                  value={status.runtime.id}
                  disabled={reason !== null}
                >
                  {status.runtime.name}
                  {reason ? ` (${reasonText(reason)})` : ''}
                </option>
              );
            })}
          </select>
          {selected && (
            <p className={styles.hint} data-availability={selected.availability}>
              {t(`runtime.availability.${selected.availability}`)}
              {selected.version ? ` · ${selected.version}` : ''}
              {selected.notice ? ` · ${t(`runtime.notice.${selected.notice}`)}` : ''}
            </p>
          )}
        </div>
      )}

      {selected && <ModelField status={selected} modelId={modelId} onModelChange={onModelChange} />}
    </>
  );
}
