import type { ReactNode } from 'react';
import { Badge, type BadgeTone } from '@/components/ui/Badge';
import { CollapsibleSection } from '@/components/ui/CollapsibleSection';

interface Props {
  title: string;
  /** What the panel says about itself: its state, always in view even while it is folded. */
  tag?: string;
  tone?: BadgeTone;
  defaultOpen?: boolean;
  children: ReactNode;
}

/** A panel of the spec's pages: a title, its state as a badge, and a body that can be folded. */
export function PanelSection({
  title,
  tag,
  tone = 'neutral',
  defaultOpen = true,
  children,
}: Props) {
  return (
    <CollapsibleSection
      title={title}
      variant="flat"
      defaultOpen={defaultOpen}
      {...(tag
        ? {
            aside: (
              <Badge dot tone={tone}>
                {tag}
              </Badge>
            ),
          }
        : {})}
    >
      {children}
    </CollapsibleSection>
  );
}
