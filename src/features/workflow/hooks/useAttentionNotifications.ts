import { useEffect, useRef } from 'react';
import { useT } from '@/i18n/I18nProvider';
import { chime, notifyUser } from '@/lib/tauri/notifications';
import { planExcerpt } from '../model/notification';
import { failureText } from '../model/status';
import {
  getRun,
  listPendingInteractions,
  subscribeToWorkflowEvents,
} from '../services/workflowService';

/** Where to go when the person comes back: the run whose agent is waiting for them. */
export interface AttentionTarget {
  workflowId: string;
  executionId: string;
}

/**
 * When something needs the person while the app is in the background, shows a system
 * notification (with sound): an agent asks something (with the start of its plan), a workflow
 * run fails, or a workflow run completes (with whether there is code to review). It remembers
 * where it happened. When the person comes back to
 * the app, `onReturn` is called once with that run, so they land on the question instead of
 * having to look for it. Nothing is notified while the app is in front: the question is already
 * on screen.
 */
export function useAttentionNotifications(onReturn: (target: AttentionTarget) => void): void {
  const t = useT();
  const waiting = useRef<AttentionTarget | undefined>(undefined);
  const go = useRef(onReturn);
  // The texts are worded when the event arrives, in the language of that moment.
  const words = useRef(t);

  useEffect(() => {
    go.current = onReturn;
    words.current = t;
  });

  useEffect(() => {
    let stop: (() => void) | undefined;
    let disposed = false;
    subscribeToWorkflowEvents((event) => {
      const t = words.current;
      if (
        event.kind === 'interaction_detected' ||
        event.kind === 'completed' ||
        event.kind === 'failed'
      ) {
        chime();
      }
      // In front, the person sees it happen: the sound and banner still tell them, but there
      // is nowhere to bring them back to.
      const away = !document.hasFocus();
      if (event.kind === 'completed' || event.kind === 'failed') {
        if (away)
          waiting.current = { workflowId: event.workflowId, executionId: event.executionId };
        void getRun(event.executionId)
          .then((run) => {
            const name = run?.workflow.name ?? '';
            if (event.kind === 'failed') {
              const why = run ? failureText(t, run) : '';
              return notifyUser(
                `✕ ${t('workflow.notify.failed', { name })}`,
                why !== '' ? why : event.message,
              );
            }
            const review = run?.integration.status === 'changes_available';
            return notifyUser(
              `✓ ${t('workflow.notify.completed', { name })}`,
              t(review ? 'workflow.notify.review' : 'workflow.notify.done'),
            );
          })
          .catch(() => undefined);
        return;
      }
      if (event.kind !== 'interaction_detected') return;
      if (away) waiting.current = { workflowId: event.workflowId, executionId: event.executionId };
      // The question alone is not enough to decide on: the notification carries the start of
      // the plan the agent wrote, and the panel Atlas opens on return has all of it.
      void listPendingInteractions(event.workspaceId)
        .then((pending) => pending.find((p) => p.workflowExecutionId === event.executionId))
        .catch(() => undefined)
        .then((found) => {
          const excerpt = found ? planExcerpt(found.document) : '';
          const title = found
            ? `⚠ ${t('interaction.notify.title')} · ${found.stepLabel}`
            : `⚠ ${t('interaction.notify.title')}`;
          const body = excerpt
            ? `${event.message}\n\n📋 ${t('interaction.notify.plan')}: ${excerpt}`
            : event.message;
          return notifyUser(title, body);
        });
    })
      .then((unlisten) => {
        if (disposed) unlisten();
        else stop = unlisten;
      })
      .catch(() => {
        // Notifications are best-effort.
      });
    const back = () => {
      if (document.visibilityState === 'hidden' || !document.hasFocus()) return;
      const target = waiting.current;
      waiting.current = undefined;
      if (target) go.current(target);
    };
    window.addEventListener('focus', back);
    document.addEventListener('visibilitychange', back);
    return () => {
      disposed = true;
      stop?.();
      window.removeEventListener('focus', back);
      document.removeEventListener('visibilitychange', back);
    };
  }, []);
}
