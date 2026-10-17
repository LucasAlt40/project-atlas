import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CommandError, type ImportPreviewDto } from '@/lib/tauri/commands';
import { mockBackend, renderWithProviders, workspace } from '@/test/fixtures';
import {
  externalItem,
  importPreview,
  modelAssessment,
  previewItem,
} from '@/test/portabilityFixtures';
import { importConfigs, pickConfigFile, previewConfigImport } from '../services/portabilityService';
import { ImportDialog } from './ImportDialog';

vi.mock('../services/portabilityService');
vi.mock('@/features/agents/services/catalogService');
vi.mock('@/features/settings/services/settingsService');
vi.mock('@/features/usage/services/usageService');
vi.mock('@/features/workspace/services/workspaceService');

const FILE = { fileName: 'architect.atlas.json', text: '{"format":"atlas-config"}' };

const reviewer = previewItem('personality-1', 'personality', 'Reviewer', {
  root: false,
  facts: [{ label: 'tags', value: 'review' }],
  text: { value: 'You are Reviewer. Be strict.', truncated: false },
});
const architect = (extra: Parameters<typeof previewItem>[3] = {}) =>
  previewItem('agent-2', 'agent', 'Architect', {
    dependsOn: ['personality-1'],
    facts: [
      { label: 'runtime', value: 'OpenCode CLI' },
      { label: 'model', value: 'opencode/big-pickle' },
      { label: 'permissions', value: 'read_only' },
      { label: 'isolation', value: 'off' },
      { label: 'contract', value: '' },
    ],
    model: modelAssessment('available'),
    ...extra,
  });

/** Opens the dialog, picks a file and waits for the review of `preview`. */
async function review(preview: ImportPreviewDto, options: { workspaces?: boolean } = {}) {
  mockBackend({
    workspaces: options.workspaces ? [workspace('ws-1', 'Atlas', '/atlas')] : [],
    selectedWorkspaceId: options.workspaces ? 'ws-1' : null,
  });
  vi.mocked(pickConfigFile).mockResolvedValue(FILE);
  vi.mocked(previewConfigImport).mockResolvedValue(preview);
  const user = userEvent.setup();
  const onClose = vi.fn();
  const onImported = vi.fn();
  renderWithProviders(<ImportDialog onClose={onClose} onImported={onImported} />);
  await user.click(await screen.findByRole('button', { name: /Choose file/ }));
  await screen.findByText('File: architect.atlas.json');
  return { user, onClose, onImported };
}

const importButton = () => screen.getByRole('button', { name: 'Import' });

describe('ImportDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('says first that importing runs nothing and grants nothing, and waits for a file', async () => {
    mockBackend({});
    renderWithProviders(<ImportDialog onClose={vi.fn()} />);

    expect(await screen.findByText(/Importing only adds definitions/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Choose file/ })).toBeEnabled();
    expect(previewConfigImport).not.toHaveBeenCalled();
  });

  it('stays where it was when the person cancels the operating system’s dialog', async () => {
    mockBackend({});
    vi.mocked(pickConfigFile).mockResolvedValue(null);
    const user = userEvent.setup();
    renderWithProviders(<ImportDialog onClose={vi.fn()} />);

    await user.click(await screen.findByRole('button', { name: /Choose file/ }));

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /Choose file/ })).toBeEnabled();
    });
    expect(previewConfigImport).not.toHaveBeenCalled();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('shows what the core said was wrong with a file, in words, and lets the person try another', async () => {
    mockBackend({});
    vi.mocked(pickConfigFile).mockResolvedValue({ fileName: 'x.json', text: 'nope' });
    vi.mocked(previewConfigImport).mockRejectedValue(
      new CommandError('preview_config_import', {
        code: 'import_unsupported_version',
        params: { found: '9', supported: '1' },
        detail: null,
      }),
    );
    const user = userEvent.setup();
    renderWithProviders(<ImportDialog onClose={vi.fn()} />);

    await user.click(await screen.findByRole('button', { name: /Choose file/ }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'This file comes from a newer Atlas (format 9; this one reads up to 1). Update Atlas to import it.',
    );
    expect(screen.getByRole('button', { name: /Choose file/ })).toBeEnabled();
  });

  it('shows every item with its kind, its content and the instructions an agent would get', async () => {
    const { user } = await review(importPreview([reviewer, architect()]));

    const list = screen.getByRole('list');
    const agent = within(list).getByRole('listitem', { name: 'Architect' });
    expect(within(agent).getByText('Agent')).toBeInTheDocument();
    expect(within(agent).getByText('Exported')).toBeInTheDocument();
    expect(within(agent).getByText('opencode/big-pickle', { selector: 'dd' })).toBeInTheDocument();
    expect(within(agent).getByText('Needs: Reviewer')).toBeInTheDocument();
    const dependency = within(list).getByRole('listitem', { name: 'Reviewer' });
    expect(within(dependency).getByText('Needed by another')).toBeInTheDocument();
    // A dependency with nothing wrong waits folded until it is opened.
    await user.click(within(dependency).getByRole('button', { name: 'Expand Reviewer' }));
    await user.click(within(dependency).getByText('Read the content of Reviewer'));
    expect(within(dependency).getByText('You are Reviewer. Be strict.')).toBeVisible();
    expect(screen.getByText('2 new · 0 replaced · 0 reused · 0 left out')).toBeInTheDocument();
  });

  it('imports what was shown, with what was decided, and then reports it', async () => {
    const { user, onImported } = await review(importPreview([reviewer, architect()]));
    vi.mocked(importConfigs).mockResolvedValue({
      items: [
        {
          ref: 'personality-1',
          kind: 'personality',
          name: 'Reviewer',
          id: 'custom-1',
          result: 'created',
          renamedTo: null,
          issues: [],
          model: null,
        },
        {
          ref: 'agent-2',
          kind: 'agent',
          name: 'Architect',
          id: 'agent-1',
          result: 'created',
          renamedTo: 'Architect (2)',
          issues: [
            {
              code: 'permissions_lowered',
              severity: 'warning',
              params: { requested: 'developer' },
            },
          ],
          model: {
            originalRuntime: 'opencode',
            originalModel: 'opencode/retired',
            runtime: 'opencode',
            model: 'opencode/alt',
          },
        },
      ],
    });

    await user.click(importButton());

    expect(importConfigs).toHaveBeenCalledWith(FILE.text, {
      workspaceId: null,
      decisions: { 'personality-1': 'create', 'agent-2': 'create' },
      models: {},
      bindings: {},
      keepPermissionProfiles: false,
    });
    expect(await screen.findByRole('status')).toHaveTextContent(
      'Import complete: 2 added or replaced.',
    );
    expect(
      screen.getByText('“Architect” was taken, so it was saved as “Architect (2)”.'),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        'Model changed from opencode · opencode/retired to opencode · opencode/alt.',
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText('Imported able only to read (the file asked for developer).'),
    ).toBeInTheDocument();
    expect(onImported).toHaveBeenCalledTimes(1);
  });

  it('shows why the core refused and keeps the choices, so nothing is lost and nothing was changed', async () => {
    const { user, onImported } = await review(importPreview([reviewer, architect()]));
    vi.mocked(importConfigs).mockRejectedValue(
      new CommandError('import_configs', {
        code: 'import_rejected',
        params: { reason: 'model_invalid', item: 'Architect' },
        detail: null,
      }),
    );

    await user.click(importButton());

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Nothing was imported: “Architect” cannot be created because its model id is not acceptable.',
    );
    expect(importButton()).toBeEnabled();
    expect(screen.getByRole('listitem', { name: 'Architect' })).toBeInTheDocument();
    expect(onImported).not.toHaveBeenCalled();
  });

  describe('models', () => {
    const missing = architect({ model: modelAssessment('model_missing') });

    it('says the model is not offered here and keeps the original unless one is chosen', async () => {
      const { user } = await review(importPreview([reviewer, missing]));

      expect(
        screen.getByText(/OpenCode CLI does not offer “opencode\/retired” on this machine/),
      ).toBeInTheDocument();
      expect(
        screen.getByText('Exported with: OpenCode CLI (OpenCode) · opencode/retired'),
      ).toBeInTheDocument();
      expect(
        screen.getByText(
          'It will be imported with the original model and needs setting up before it runs.',
        ),
      ).toBeInTheDocument();
      expect(importButton()).toBeEnabled();
      vi.mocked(importConfigs).mockResolvedValue({ items: [] });

      await user.click(importButton());

      expect(vi.mocked(importConfigs).mock.calls[0]?.[1].models).toEqual({});
    });

    it('sends the alternative the person picked', async () => {
      const { user } = await review(importPreview([reviewer, missing]));
      vi.mocked(importConfigs).mockResolvedValue({ items: [] });

      await user.selectOptions(
        screen.getByRole('combobox', { name: 'Runtime and model for Architect' }),
        'opencode/alt',
      );
      await user.click(importButton());

      expect(vi.mocked(importConfigs).mock.calls[0]?.[1].models).toEqual({
        'agent-2': { runtimeId: 'opencode', modelId: 'opencode/alt' },
      });
    });

    it('does not let an agent whose runtime cannot be kept in without another one', async () => {
      const lost = architect({ model: modelAssessment('runtime_missing') });
      const { user } = await review(importPreview([reviewer, lost]));

      expect(
        screen.getByText(
          'This Atlas has no runtime “OpenCode CLI”. Choose another runtime and model.',
        ),
      ).toBeInTheDocument();
      expect(importButton()).toBeDisabled();
      expect(screen.getByText('Choose a runtime and model for Architect.')).toBeInTheDocument();
      await user.selectOptions(
        screen.getByRole('combobox', { name: 'Runtime and model for Architect' }),
        'opencode/big-pickle',
      );
      expect(importButton()).toBeEnabled();
    });

    it('does not ask about the model of an agent the person leaves out', async () => {
      const lost = architect({ model: modelAssessment('runtime_missing') });
      const { user } = await review(importPreview([reviewer, lost]));

      await user.selectOptions(
        screen.getByRole('combobox', { name: 'What to do with Architect' }),
        'skip',
      );

      expect(
        screen.queryByRole('combobox', { name: 'Runtime and model for Architect' }),
      ).toBeNull();
      expect(importButton()).toBeEnabled();
    });
  });

  describe('conflicts', () => {
    const conflicting = architect({
      existing: { id: 'agent-local', name: 'Architect' },
      allowed: ['skip', 'create', 'use_existing', 'replace'],
    });

    it('says what already exists, imports as new by default and never overwrites on its own', async () => {
      const { user } = await review(importPreview([reviewer, conflicting]));
      vi.mocked(importConfigs).mockResolvedValue({ items: [] });

      expect(
        screen.getByText('You already have “Architect”, with different content.'),
      ).toBeInTheDocument();
      expect(
        screen.getByText(
          'It will be added as a new one; the name gets a number because it is taken.',
        ),
      ).toBeInTheDocument();
      await user.click(importButton());

      expect(vi.mocked(importConfigs).mock.calls[0]?.[1].decisions['agent-2']).toBe('create');
    });

    it('replaces only after the person confirms it', async () => {
      const { user } = await review(importPreview([reviewer, conflicting]));
      vi.mocked(importConfigs).mockResolvedValue({ items: [] });
      await user.selectOptions(
        screen.getByRole('combobox', { name: 'What to do with Architect' }),
        'replace',
      );

      expect(screen.getByText(/Replacing overwrites “Architect”/)).toBeInTheDocument();
      expect(importButton()).toBeDisabled();
      await user.click(screen.getByRole('checkbox', { name: 'I confirm: overwrite “Architect”' }));
      expect(importButton()).toBeEnabled();
      await user.click(importButton());

      expect(vi.mocked(importConfigs).mock.calls[0]?.[1].decisions['agent-2']).toBe('replace');
    });

    it('forgets a confirmation when the decision changes', async () => {
      const { user } = await review(importPreview([reviewer, conflicting]));
      const decision = screen.getByRole('combobox', { name: 'What to do with Architect' });
      await user.selectOptions(decision, 'replace');
      await user.click(screen.getByRole('checkbox', { name: 'I confirm: overwrite “Architect”' }));
      await user.selectOptions(decision, 'create');
      await user.selectOptions(decision, 'replace');

      expect(
        screen.getByRole('checkbox', { name: 'I confirm: overwrite “Architect”' }),
      ).not.toBeChecked();
      expect(importButton()).toBeDisabled();
    });

    it('does not let something be imported without what it needs', async () => {
      const { user } = await review(importPreview([reviewer, architect()]));

      await user.click(screen.getByRole('button', { name: 'Expand Reviewer' }));
      await user.selectOptions(
        screen.getByRole('combobox', { name: 'What to do with Reviewer' }),
        'skip',
      );

      expect(importButton()).toBeDisabled();
      expect(
        screen.getByText('Architect needs something you chose not to import.'),
      ).toBeInTheDocument();
    });

    it('offers nothing to import when the file is only what is already here', async () => {
      const same = architect({
        existing: { id: 'agent-local', name: 'Architect' },
        identical: true,
        defaultDecision: 'use_existing',
        allowed: ['skip', 'create', 'use_existing', 'replace'],
      });
      await review(importPreview([same], { external: [] }));

      expect(
        screen.getByText('You already have “Architect”, and it is the same.'),
      ).toBeInTheDocument();
      expect(screen.getByText('Nothing would be imported.')).toBeInTheDocument();
      expect(importButton()).toBeDisabled();
    });
  });

  describe('what the file does not carry', () => {
    const flow = previewItem('workflow-1', 'workflow', 'Feature flow', {
      dependsOn: ['agent-1', 'agent-2'],
      steps: [
        {
          label: 'Implement',
          kind: 'agent',
          agent: 'agent-1',
          instructions: null,
          readOnly: false,
        },
      ],
    });
    const preview = importPreview([flow], {
      external: [
        externalItem('agent-1', 'agent', 'Implementer', {
          suggested: 'local-a',
          candidates: [
            { id: 'local-a', name: 'Implementer' },
            { id: 'local-b', name: 'Other' },
          ],
          usedBy: ['workflow-1'],
        }),
        externalItem('agent-2', 'agent', 'Checker', {
          candidates: [{ id: 'local-b', name: 'Other' }],
          usedBy: ['workflow-1'],
        }),
      ],
    });

    it('asks which of the person’s own each is, offers the same name without applying it silently', async () => {
      const { user } = await review(preview, { workspaces: true });
      vi.mocked(importConfigs).mockResolvedValue({ items: [] });

      const implementer = screen.getByRole('combobox', { name: /Implementer/ });
      expect(implementer).toHaveValue('local-a');
      expect(within(implementer).getByText('Implementer — same name')).toBeInTheDocument();
      expect(screen.getByRole('combobox', { name: /Checker/ })).toHaveValue('');
      expect(importButton()).toBeDisabled();
      expect(screen.getByText('Choose which of yours “Checker” is.')).toBeInTheDocument();
      await user.selectOptions(screen.getByRole('combobox', { name: /Checker/ }), 'local-b');
      await user.click(importButton());

      expect(vi.mocked(importConfigs).mock.calls[0]?.[1]).toMatchObject({
        workspaceId: 'ws-1',
        bindings: { 'agent-1': 'local-a', 'agent-2': 'local-b' },
      });
    });
  });

  describe('workflows', () => {
    const flow = previewItem('workflow-1', 'workflow', 'Feature flow', {
      facts: [
        { label: 'steps', value: '3' },
        { label: 'mode', value: 'custom' },
      ],
      issues: [{ code: 'workflow_draft', severity: 'warning', params: { errors: '2' } }],
    });

    it('needs a workspace before a workflow can be imported', async () => {
      await review(importPreview([flow]), { workspaces: false });
      expect(importButton()).toBeDisabled();
      expect(screen.getByText('Choose the workspace the workflows go to.')).toBeInTheDocument();
      expect(screen.getByText(/It has problems that stop it from running/)).toBeInTheDocument();
      expect(previewConfigImport).toHaveBeenLastCalledWith(FILE.text, null);
    });

    it('reads the file again against the workspace the person picks, keeping what was chosen', async () => {
      const { user } = await review(importPreview([flow]), { workspaces: true });
      await user.selectOptions(
        screen.getByRole('combobox', { name: /Workspace for the workflows/ }),
        '',
      );
      expect(importButton()).toBeDisabled();
      vi.mocked(previewConfigImport).mockClear();

      await user.selectOptions(
        screen.getByRole('combobox', { name: /Workspace for the workflows/ }),
        'ws-1',
      );

      await waitFor(() => {
        expect(previewConfigImport).toHaveBeenCalledWith(FILE.text, 'ws-1');
      });
      await waitFor(() => {
        expect(importButton()).toBeEnabled();
      });
    });

    it('starts from the workspace that is open and imports into it', async () => {
      const { user } = await review(importPreview([flow]), { workspaces: true });
      vi.mocked(importConfigs).mockResolvedValue({ items: [] });

      expect(previewConfigImport).toHaveBeenLastCalledWith(FILE.text, 'ws-1');
      expect(screen.getByRole('combobox', { name: /Workspace for the workflows/ })).toHaveValue(
        'ws-1',
      );
      await user.click(importButton());

      expect(vi.mocked(importConfigs).mock.calls[0]?.[1].workspaceId).toBe('ws-1');
    });
  });

  describe('permissions and warnings', () => {
    const editing = architect({
      issues: [
        { code: 'grants_editing', severity: 'warning', params: {} },
        { code: 'possible_secret', severity: 'warning', params: {} },
        { code: 'invalid_spec', severity: 'error', params: { reason: 'model_invalid' } },
      ],
      allowed: ['skip', 'use_existing'],
      defaultDecision: 'skip',
    });

    it('imports agents that may edit as readers unless the person keeps the permission', async () => {
      const ok = architect({
        issues: [{ code: 'grants_editing', severity: 'warning', params: {} }],
      });
      const { user } = await review(importPreview([reviewer, ok], { grantsEditing: true }));
      vi.mocked(importConfigs).mockResolvedValue({ items: [] });

      const keep = screen.getByRole('checkbox', {
        name: /Keep the permission to edit files that the agents were exported with/,
      });
      expect(keep).not.toBeChecked();
      expect(
        screen.getByText('This agent asks for the permission to edit files.'),
      ).toBeInTheDocument();
      await user.click(keep);
      await user.click(importButton());

      expect(vi.mocked(importConfigs).mock.calls[0]?.[1].keepPermissionProfiles).toBe(true);
    });

    it('shows secrets and invalid items, and cannot import what the core said is invalid', async () => {
      await review(importPreview([reviewer, editing]));

      expect(screen.getByText(/A text here looks like a credential/)).toBeInTheDocument();
      expect(
        screen.getByText('It cannot be imported: its model id is not acceptable.'),
      ).toBeInTheDocument();
      const decision = screen.getByRole('combobox', { name: 'What to do with Architect' });
      expect(within(decision).queryByRole('option', { name: 'Import as new' })).toBeNull();
      expect(decision).toHaveValue('skip');
    });
  });
});
