import { useEffect, useRef } from 'react';
import { useI18n } from '@/i18n/I18nProvider';
import type { SpecFileOutlineDto } from '@/lib/tauri/commands';
import styles from './Specs.module.css';

interface Props {
  outline: SpecFileOutlineDto;
  /** A requirement to bring into view and mark (from a finding that points at it). */
  focus?: string | null | undefined;
}

/** The requirements and scenarios of one spec file, as cards: the way they are read and judged. */
export function SpecsOutline({ outline, focus }: Props) {
  const { t } = useI18n();
  const focused = useRef<HTMLElement | null>(null);

  useEffect(() => {
    focused.current?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }, [focus]);

  const wanted = (focus ?? '')
    .replace(/^requirement:\s*/i, '')
    .trim()
    .toLowerCase();

  return (
    <div className={styles.outline}>
      {outline.purpose && (
        <section className={styles.purpose}>
          <h4>{t('specs.doc.purpose')}</h4>
          <p>{outline.purpose}</p>
        </section>
      )}
      {outline.requirements.length === 0 && (
        <p className={styles.muted}>{t('specs.doc.noRequirements')}</p>
      )}
      {outline.requirements.map((requirement) => {
        const isFocus = wanted !== '' && requirement.name.toLowerCase() === wanted;
        return (
          <article
            key={`${String(requirement.line)}-${requirement.name}`}
            className={styles.requirement}
            data-focus={isFocus}
            data-operation={requirement.operation}
            ref={
              isFocus
                ? (node) => {
                    focused.current = node;
                  }
                : undefined
            }
          >
            <header>
              <span className={styles.operation}>{requirement.operation}</span>
              <h4>{requirement.name}</h4>
              <span className={styles.line}>{t('specs.doc.line', { line: requirement.line })}</span>
            </header>
            {requirement.text && <p className={styles.requirementText}>{requirement.text}</p>}
            {requirement.scenarios.length > 0 && (
              <div className={styles.scenarios}>
                <h5>{t('specs.doc.scenarios', { count: requirement.scenarios.length })}</h5>
                {requirement.scenarios.map((scenario) => (
                  <div
                    key={`${String(scenario.line)}-${scenario.name}`}
                    className={styles.scenario}
                  >
                    <h6>{scenario.name}</h6>
                    <ol>
                      {scenario.steps.map((step, index) => (
                        <li key={index}>
                          <span className={styles.keyword} data-keyword={step.keyword}>
                            {step.keyword}
                          </span>
                          <span>{step.text}</span>
                        </li>
                      ))}
                    </ol>
                  </div>
                ))}
              </div>
            )}
          </article>
        );
      })}
    </div>
  );
}
