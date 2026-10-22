import type { ReactNode } from 'react';
import { Badge, type BadgeTone } from '@/components/ui/Badge';
import type { Tone } from '../model/status';

/** One map from the tone a status has in Specs to the badge that says so. */
const TONE: Record<Tone, BadgeTone> = {
  neutral: 'neutral',
  info: 'info',
  running: 'running',
  success: 'success',
  warning: 'warning',
  danger: 'danger',
};

export function StatusBadge({ tone, children }: { tone: Tone; children: ReactNode }) {
  return (
    <Badge dot tone={TONE[tone]}>
      {children}
    </Badge>
  );
}
