import type { TranslationKey } from '@/i18n';
import type { ToolPlanItemDto, ToolUnavailableDto } from '@/lib/tauri/commands';
import type { WorkflowTemplate } from '../types';

/**
 * What the screen says about a template before it runs. The three that ship with Atlas have their
 * goal and outputs translated; any other template (the older variations, the ones a person saved)
 * shows the text it carries.
 */
const FEATURED_COPY: Record<string, { goal: TranslationKey; outputs: TranslationKey[] }> = {
  software_feature: {
    goal: 'workflow.templateGoal.software_feature',
    outputs: [
      'workflow.templateOutput.software_feature.0',
      'workflow.templateOutput.software_feature.1',
      'workflow.templateOutput.software_feature.2',
    ],
  },
  bug_fix: {
    goal: 'workflow.templateGoal.bug_fix',
    outputs: [
      'workflow.templateOutput.bug_fix.0',
      'workflow.templateOutput.bug_fix.1',
      'workflow.templateOutput.bug_fix.2',
    ],
  },
  code_review: {
    goal: 'workflow.templateGoal.code_review',
    outputs: ['workflow.templateOutput.code_review.0', 'workflow.templateOutput.code_review.1'],
  },
};

export function goalKey(template: WorkflowTemplate): TranslationKey | null {
  return template.kind === 'builtin' ? (FEATURED_COPY[template.id]?.goal ?? null) : null;
}

/** The translated key of an output, when the template's outputs are the ones Atlas translated. */
export function outputKey(template: WorkflowTemplate, index: number): TranslationKey | null {
  if (template.kind !== 'builtin') return null;
  return FEATURED_COPY[template.id]?.outputs[index] ?? null;
}

export const REASON_KEY: Record<ToolUnavailableDto['kind'], TranslationKey> = {
  no_connection: 'workflow.template.toolReason.no_connection',
  disabled: 'workflow.template.toolReason.disabled',
  needs_secret: 'workflow.template.toolReason.needs_secret',
  not_connected: 'workflow.template.toolReason.not_connected',
  needs_discovery: 'workflow.template.toolReason.needs_discovery',
  no_reading_tools: 'workflow.template.toolReason.no_reading_tools',
  refused: 'workflow.template.toolReason.refused',
};

/** Whether starting the workflow with the tools option on would grant anything. */
export function canGrantTools(plan: readonly ToolPlanItemDto[]): boolean {
  return plan.some((item) => item.reason === null && item.tools !== null);
}
