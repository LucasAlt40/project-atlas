import { useState } from 'react';
import { Button } from '@/components/ui/Button';
import { useDrafts } from '../hooks/useDrafts';
import { SendAnswersDialog } from './SendAnswersDialog';
import { useI18n } from '@/i18n/I18nProvider';
import { categoryText, stageName } from '../model/messages';
import type { FindingStatus, SpecFinding } from '../types';
import styles from './Specs.module.css';

interface Props {
  findings: SpecFinding[];
  busy: boolean;
  onResolve: (findingId: string, status: FindingStatus, note: string | null) => void;
  /** Takes the person to the place in the document the finding points at. */
  onShow?: ((finding: SpecFinding) => void) | undefined;
}

/** What the reviewing agent found: opinions with their evidence, for the person to decide on. */
export function Findings({ findings, busy, onResolve, onShow }: Props) {
  const { t } = useI18n();
  if (findings.length === 0) return null;
  return (
    <ul className={styles.findings} aria-label={t('specs.findings.label')}>
      {findings.map((finding) => (
        <li key={finding.id}>
          <FindingCard finding={finding} busy={busy} onResolve={onResolve} onShow={onShow} />
        </li>
      ))}
    </ul>
  );
}

function FindingCard({
  finding,
  busy,
  draft = '',
  onDraft,
  onResolve,
  onShow,
}: Omit<Props, 'findings'> & {
  finding: SpecFinding;
  /** What the person has written for the question so far (kept until they send them all). */
  draft?: string;
  onDraft?: (text: string) => void;
}) {
  const { t } = useI18n();
  const [note, setNote] = useState('');
  const [accepting, setAccepting] = useState(false);
  const open = finding.status === 'open';
  return (
    <article
      className={styles.finding}
      data-severity={finding.severity}
      data-status={finding.status}
    >
      <header>
        <span className={styles.categoryChip} data-category={finding.category}>
          {categoryText(t, finding)}
        </span>
        <span className={styles.findingSeverity}>
          {t(`specs.finding.severity.${finding.severity}`)}
        </span>
        {finding.artifactId && (
          <span className={styles.mono}>
            {stageName(t, finding.artifactId)}
            {finding.reference ? ` · ${finding.reference}` : ''}
          </span>
        )}
        {!open && (
          <span className={styles.findingState}>{t(`specs.finding.status.${finding.status}`)}</span>
        )}
      </header>
      <h4>{finding.title}</h4>
      <div className={styles.diagnosis}>
        <p>{finding.description}</p>
        {finding.suggestion && (
          <p className={styles.suggestion}>
            <strong>{t('specs.finding.suggestion')}</strong> {finding.suggestion}
          </p>
        )}
      </div>
      {open && finding.question && onDraft && (
        <div className={styles.answerBox}>
          <label className={styles.field}>
            <span className={styles.questionLabel}>{finding.question}</span>
            <textarea
              rows={3}
              value={draft}
              maxLength={2000}
              placeholder={t('specs.finding.answerPlaceholder')}
              onChange={(event) => {
                onDraft(event.target.value);
              }}
            />
          </label>
          <small className={styles.hint}>
            {draft.trim() === '' ? t('specs.finding.draftHint') : t('specs.finding.draftKept')}
          </small>
        </div>
      )}
      {(finding.evidence || finding.rationale) && (
        <details className={styles.evidence}>
          <summary>{t('specs.finding.evidence')}</summary>
          {finding.evidence && <blockquote>{finding.evidence}</blockquote>}
          {finding.rationale && <p className={styles.muted}>{finding.rationale}</p>}
        </details>
      )}
      <p className={styles.opinion}>{t('specs.finding.opinion')}</p>
      {finding.resolutionNote && (
        <p className={styles.resolution}>
          <strong>{t('specs.finding.decision')}</strong> {finding.resolutionNote}
        </p>
      )}
      <div className={styles.row}>
        {onShow && finding.artifactId && (
          <Button
            variant="ghost"
            onClick={() => {
              onShow(finding);
            }}
          >
            {t('specs.finding.show')}
          </Button>
        )}
        {open && !accepting && (
          <>
            <Button
              variant="ghost"
              disabled={busy}
              onClick={() => {
                onResolve(finding.id, 'resolved', null);
              }}
            >
              {t('specs.finding.resolve')}
            </Button>
            <Button
              variant="ghost"
              disabled={busy}
              onClick={() => {
                onResolve(finding.id, 'dismissed', null);
              }}
            >
              {t('specs.finding.dismiss')}
            </Button>
            <Button
              variant="ghost"
              disabled={busy}
              onClick={() => {
                setAccepting(true);
              }}
            >
              {t('specs.finding.accept')}
            </Button>
          </>
        )}
        {!open && (
          <Button
            variant="ghost"
            disabled={busy}
            onClick={() => {
              onResolve(finding.id, 'open', null);
            }}
          >
            {t('specs.finding.reopen')}
          </Button>
        )}
      </div>
      {open && accepting && (
        <div className={styles.answerBox}>
          <label className={styles.field}>
            <span>{t('specs.finding.acceptWhy')}</span>
            <textarea
              rows={2}
              value={note}
              maxLength={2000}
              onChange={(event) => {
                setNote(event.target.value);
              }}
            />
          </label>
          <div className={styles.row}>
            <Button
              disabled={busy || note.trim() === ''}
              onClick={() => {
                onResolve(finding.id, 'accepted_risk', note.trim());
              }}
            >
              {t('specs.finding.acceptConfirm')}
            </Button>
            <Button
              variant="secondary"
              onClick={() => {
                setAccepting(false);
              }}
            >
              {t('common.cancel')}
            </Button>
          </div>
        </div>
      )}
    </article>
  );
}

const WEIGHT = { high: 0, medium: 1, low: 2 } as const;

/** What needs the person first: the ones that ask them something, then the gravest. */
function byUrgency(a: SpecFinding, b: SpecFinding): number {
  const asked = Number(Boolean(b.question)) - Number(Boolean(a.question));
  return asked !== 0 ? asked : WEIGHT[a.severity] - WEIGHT[b.severity];
}

/**
 * The open findings as a list beside the one being read: the person goes down the list, not down
 * a page. The ones that ask a question come first, and when one is dealt with the next is shown.
 * Every finding keeps its own draft while another is on screen.
 */
export function FindingsBrowser({
  specId,
  findings,
  busy,
  onSendAnswers,
  onResolve,
  onShow,
}: Omit<Props, 'findings'> & {
  specId: string;
  findings: SpecFinding[];
  /** All the answers the person wrote, together: the documents are revised once, with all of them. */
  onSendAnswers: (answers: { findingId: string; answer: string }[]) => Promise<boolean>;
}) {
  const { t } = useI18n();
  const { drafts, set, clear } = useDrafts(specId);
  const [reviewing, setReviewing] = useState(false);
  const items = [...findings].sort(byUrgency);
  const [selected, setSelected] = useState<{ id: string; index: number } | null>(null);
  const found = items.findIndex((f) => f.id === selected?.id);
  // The one on screen was settled: the one that took its place in the list is the next to read.
  const index = found >= 0 ? found : Math.min(selected?.index ?? 0, items.length - 1);
  const current = items[index];
  const asking = items.filter((f) => f.question).length;
  const answered = items.filter((f) => f.question && (drafts[f.id] ?? '').trim() !== '');
  if (!current) return null;
  return (
    <div className={styles.browser}>
      <p className={styles.browserSummary}>
        {t('specs.findings.summary', { open: items.length, asking })}
        {asking > 0 &&
          ` · ${t('specs.findings.answered', { done: answered.length, total: asking })}`}
      </p>
      <ul className={styles.browserList} aria-label={t('specs.findings.label')}>
        {items.map((finding, position) => (
          <li key={finding.id}>
            <button
              type="button"
              className={styles.browserItem}
              data-severity={finding.severity}
              aria-current={position === index}
              data-answered={Boolean(finding.question) && (drafts[finding.id] ?? '').trim() !== ''}
              onClick={() => {
                setSelected({ id: finding.id, index: position });
              }}
            >
              <span className={styles.browserMeta}>
                <span className={styles.categoryChip} data-category={finding.category}>
                  {categoryText(t, finding)}
                </span>
                {finding.artifactId && <span>{stageName(t, finding.artifactId)}</span>}
                {finding.question &&
                  ((drafts[finding.id] ?? '').trim() !== '' ? (
                    <strong className={styles.answeredMark}>{t('specs.findings.draftMark')}</strong>
                  ) : (
                    <strong className={styles.asksYou}>{t('specs.findings.asksYou')}</strong>
                  ))}
              </span>
              <span className={styles.browserTitle}>{finding.title}</span>
            </button>
          </li>
        ))}
      </ul>
      <div className={styles.browserDetail}>
        {items.map((finding) => (
          <div key={finding.id} hidden={finding.id !== current.id}>
            <FindingCard
              finding={finding}
              busy={busy}
              draft={drafts[finding.id] ?? ''}
              onDraft={(text) => {
                set(finding.id, text);
              }}
              onResolve={onResolve}
              onShow={onShow}
            />
          </div>
        ))}
        {asking > 0 && (
          <div className={styles.answerBar}>
            <span>{t('specs.findings.answered', { done: answered.length, total: asking })}</span>
            <Button
              disabled={busy || answered.length === 0}
              onClick={() => {
                setReviewing(true);
              }}
            >
              {t('specs.findings.review', { count: answered.length })}
            </Button>
          </div>
        )}
      </div>
      {reviewing && (
        <SendAnswersDialog
          answered={answered.map((finding) => ({ finding, answer: drafts[finding.id] ?? '' }))}
          unanswered={asking - answered.length}
          busy={busy}
          onChange={set}
          onClose={() => {
            setReviewing(false);
          }}
          onSend={() => {
            const sent = answered.map((f) => ({
              findingId: f.id,
              answer: (drafts[f.id] ?? '').trim(),
            }));
            void onSendAnswers(sent).then((ok) => {
              if (ok) {
                clear(sent.map((a) => a.findingId));
                setReviewing(false);
              }
            });
          }}
        />
      )}
    </div>
  );
}
