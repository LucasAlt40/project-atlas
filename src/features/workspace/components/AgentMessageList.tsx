import { useEffect, useRef } from 'react';
import { Markdown } from '@/components/ui/Markdown';
import { failureMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import type { Message } from '../types';
import styles from './AgentCard.module.css';

interface Props {
  messages: Message[];
  /** The answer being written right now, if the agent is streaming one. */
  liveText?: string | undefined;
}

export function AgentMessageList({ messages, liveText }: Props) {
  const t = useT();
  const list = useRef<HTMLOListElement>(null);
  // Follow the conversation as it grows, like a chat.
  useEffect(() => {
    if (list.current) list.current.scrollTop = list.current.scrollHeight;
  }, [messages.length, liveText]);

  if (messages.length === 0 && !liveText) {
    return <p className={styles.empty}>{t('agent.noMessages')}</p>;
  }
  return (
    <ol ref={list} className={styles.messages} aria-label={t('agent.conversation')}>
      {messages.map((message) => (
        <li
          key={message.id}
          className={styles.message}
          data-role={message.role}
          data-failed={message.failed}
        >
          {message.role === 'user' ? (
            <p className={styles.userText}>{message.content}</p>
          ) : (
            <div className={styles.answer}>
              {message.failed ? (
                // The core sends a failure code; the words are ours, in the user's language.
                <p>
                  {t('chat.failedPrefix')} {failureMessage(t, message.failureKind)}
                </p>
              ) : (
                <Markdown>{message.content}</Markdown>
              )}
            </div>
          )}
        </li>
      ))}
      {liveText && (
        <li className={styles.message} data-role="assistant" data-streaming="true">
          <div className={styles.answer} aria-live="polite">
            <Markdown>{liveText}</Markdown>
            <span aria-hidden="true" className={styles.cursor}>
              ▍
            </span>
          </div>
        </li>
      )}
    </ol>
  );
}
