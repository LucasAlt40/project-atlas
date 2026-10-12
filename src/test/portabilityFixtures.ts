import type {
  ImportExternalDto,
  ImportItemPreviewDto,
  ImportModelAssessmentDto,
  ImportPreviewDto,
} from '@/lib/tauri/commands';

/** An item of an import preview with what a test rarely cares about filled in. */
export function previewItem(
  ref: string,
  kind: ImportItemPreviewDto['kind'],
  name: string,
  extra: Partial<ImportItemPreviewDto> = {},
): ImportItemPreviewDto {
  return {
    ref,
    kind,
    name,
    description: '',
    root: true,
    facts: [],
    text: null,
    steps: [],
    existing: null,
    identical: false,
    defaultDecision: 'create',
    allowed: ['skip', 'create'],
    dependsOn: [],
    model: null,
    issues: [],
    ...extra,
  };
}

export function modelAssessment(
  status: ImportModelAssessmentDto['status'],
  extra: Partial<ImportModelAssessmentDto> = {},
): ImportModelAssessmentDto {
  return {
    original: {
      runtimeId: 'opencode',
      runtimeName: 'OpenCode CLI',
      provider: 'OpenCode',
      modelId: 'opencode/retired',
    },
    status,
    keepAllowed: status !== 'runtime_missing' && status !== 'runtime_unsupported',
    alternatives: [
      {
        runtimeId: 'opencode',
        runtimeName: 'OpenCode CLI',
        models: [
          { id: 'opencode/big-pickle', name: 'opencode/big-pickle' },
          { id: 'opencode/alt', name: 'opencode/alt' },
        ],
      },
    ],
    ...extra,
  };
}

export function externalItem(
  ref: string,
  kind: ImportExternalDto['kind'],
  name: string,
  extra: Partial<ImportExternalDto> = {},
): ImportExternalDto {
  return {
    ref,
    kind,
    name,
    description: '',
    suggested: null,
    candidates: [],
    usedBy: [],
    ...extra,
  };
}

export function importPreview(
  items: ImportItemPreviewDto[],
  extra: Partial<ImportPreviewDto> = {},
): ImportPreviewDto {
  return {
    schemaVersion: 1,
    exportedAt: 1_700_000_000_000,
    source: { app: 'atlas', appVersion: '0.1.2' },
    roots: items.filter((i) => i.root).map((i) => i.ref),
    items,
    external: [],
    needsWorkspace: items.some((i) => i.kind === 'workflow'),
    grantsEditing: false,
    ...extra,
  };
}
