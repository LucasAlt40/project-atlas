import { useEffect, useRef } from 'react';
import { Icon } from '@/components/ui/Icon';
import { Markdown } from '@/components/ui/Markdown';
import { failureMessage } from '@/i18n/messages';
import { useT } from '@/i18n/I18nProvider';
import type { Message } from '../types';
import styles from './AgentCard.module.css';

interface Props {
  messages: Message[];
  /** The answer being written right now, if the agent is streaming one. */
  liveText?: string | undefined;
  /** Who is answering, shown above each answer. */
  agentName?: string;
}

const time = (timestamp: number) =>
  new Date(timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

function Avatar() {
  return (
    <span className={styles.avatar} aria-hidden="true">
      <Icon name="agents" size={18} />
    </span>
  );
}

function Byline({ name, at }: { name: string | undefined; at?: number }) {
  if (!name) return null;
  return (
    <span className={styles.byline}>
      <strong>{name}</strong>
      {at !== undefined && <span className={styles.time}>{time(at)}</span>}
    </span>
  );
}

export function AgentMessageList({ messages, liveText, agentName }: Props) {
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
          data-failed={message.failed && message.failureKind !== 'cancelled'}
        >
          {message.role === 'user' ? (
            <p className={styles.userText}>{message.content}</p>
          ) : (
            <>
              <Avatar />
              <div className={styles.answerColumn}>
                <Byline name={agentName} at={message.timestamp} />
                <div className={styles.answer}>
                  {message.failureKind === 'cancelled' ? (
                    // Stopped by the user: not a failure, so no "failed" wording.
                    <p>{t('chat.cancelled')}</p>
                  ) : message.failed ? (
                    // The core sends a failure code; the words are ours, in the user's language.
                    <p>
                      {t('chat.failedPrefix')} {failureMessage(t, message.failureKind)}
                    </p>
                  ) : (
                    <Markdown>{message.content}</Markdown>
                  )}
                </div>
              </div>
            </>
          )}
        </li>
      ))}
      {liveText && (
        <li className={styles.message} data-role="assistant" data-streaming="true">
          <Avatar />
          <div className={styles.answerColumn}>
            <Byline name={agentName} />
            <div className={styles.answer} aria-live="polite">
              <Markdown>{liveText}</Markdown>
              <span aria-hidden="true" className={styles.cursor}>
                ▍
              </span>
            </div>
          </div>
        </li>
      )}
    </ol>
  );
}
