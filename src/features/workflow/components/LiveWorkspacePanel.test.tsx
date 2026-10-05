import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { I18nProvider } from '@/i18n/I18nProvider';
import type { LiveWorkspaceUpdateDto, WorkflowExecutionDto } from '@/lib/tauri/commands';
import {
  fileChange,
  liveState,
  liveUpdate,
  passwordRecovery,
  run,
  withCode,
} from '@/test/workflowFixtures';
import {
  getLiveDiff,
  getLiveFile,
  getLiveWorkspace,
  refreshLiveWorkspace,
  subscribeToLiveUpdates,
} from '../services/liveWorkspaceService';
import { getDiff } from '../services/workflowService';
import { LiveWorkspacePanel } from './LiveWorkspacePanel';

vi.mock('../services/liveWorkspaceService');
vi.mock('../services/workflowService');

const w = passwordRecovery();
const running = (): WorkflowExecutionDto => withCode(run(w, 'running', {}), 'in_progress');
const completed = (): WorkflowExecutionDto =>
  withCode(run(w, 'completed', {}), 'changes_available');

/** The backend's events, sent by the test. */
let emit: (update: LiveWorkspaceUpdateDto) => void = () => undefined;
let unlisten = vi.fn();

const DIFF = [
  'diff --git a/src/a.ts b/src/a.ts',
  '--- a/src/a.ts',
  '+++ b/src/a.ts',
  '@@ -1,2 +1,2 @@',
  ' keep',
  '-old line',
  '+new line',
  '',
].join('\n');

/** An error as the commands report one: an Error carrying the coded error. */
const coded = (code: string) => Object.assign(new Error(code), { appError: { code, params: {} } });

const text = (content: string, path = 'src/a.ts') => ({
  path,
  kind: 'text' as const,
  content,
  size: content.length,
  truncated: false,
});

function show(
  state: ReturnType<typeof liveState> | null,
  options: {
    current?: WorkflowExecutionDto;
    snapshot?: () => Promise<ReturnType<typeof liveState> | null>;
  } = {},
) {
  vi.mocked(getLiveWorkspace).mockReset();
  if (options.snapshot) vi.mocked(getLiveWorkspace).mockImplementation(options.snapshot);
  else vi.mocked(getLiveWorkspace).mockResolvedValue(state);
  vi.mocked(subscribeToLiveUpdates).mockImplementation((handler) => {
    emit = (update) => {
      act(() => {
        handler(update);
      });
    };
    return Promise.resolve(unlisten);
  });
  vi.mocked(getLiveDiff).mockResolvedValue(DIFF);
  vi.mocked(getLiveFile).mockResolvedValue(text('keep\nnew line\n'));
  vi.mocked(getDiff).mockResolvedValue(DIFF);
  vi.mocked(refreshLiveWorkspace).mockReset();
  const current = options.current ?? running();
  const view = render(
    <I18nProvider language="en-US">
      <LiveWorkspacePanel run={current} />
    </I18nProvider>,
  );
  return {
    ...view,
    rerunWith: (next: WorkflowExecutionDto) => {
      view.rerender(
        <I18nProvider language="en-US">
          <LiveWorkspacePanel run={next} />
        </I18nProvider>,
      );
    },
  };
}

const panel = () => screen.findByRole('region', { name: 'Live workspace' });
const files = async () => within(await screen.findByRole('group', { name: 'Changed files' }));
const FILES = [fileChange('src/a.ts'), fileChange('src/b.ts', 'added')];

beforeEach(() => {
  vi.clearAllMocks();
  unlisten = vi.fn();
});

describe('the live workspace: what it says', () => {
  it('reads the worktree and shows its changed files, totals and where it is compared from', async () => {
    show(liveState({ files: FILES }));

    const region = await panel();
    const live = within(region);

    expect(await live.findByText('LIVE')).toBeInTheDocument();
    expect(live.getByText('Agent working')).toBeInTheDocument();
    expect(
      (await files()).getByRole('button', { name: 'src/a.ts (Modified)' }),
    ).toBeInTheDocument();
    expect((await files()).getByRole('button', { name: 'src/b.ts (Added)' })).toBeInTheDocument();
    expect(live.getByLabelText('Totals against the baseline')).toHaveTextContent(
      '2 file(s) · +6 −1',
    );
    expect(region).toHaveTextContent('not your project');
    expect(region).toHaveTextContent('abc1234');
  });

  it('shows a loading state, then an empty state when the worktree matches the baseline', async () => {
    show(liveState({ files: [] }));

    expect(screen.getByRole('status')).toHaveTextContent('Reading the run’s worktree…');
    expect(await screen.findByText(/No changes yet/)).toBeInTheDocument();
    expect(screen.getByText('Nothing has changed yet.')).toBeInTheDocument();
  });

  it('shows nothing for a run that has no code worktree', async () => {
    show(null);

    await waitFor(() => {
      expect(getLiveWorkspace).toHaveBeenCalled();
    });
    expect(screen.queryByRole('region', { name: 'Live workspace' })).toBeNull();
  });

  it('shows the error when the worktree cannot be read', async () => {
    show(null, {
      snapshot: () => Promise.reject(coded('worktree_failed')),
    });

    expect((await screen.findAllByRole('alert')).length).toBeGreaterThan(0);
  });

  it.each([
    ['running', 'Agent working', 'LIVE'],
    ['waiting_for_input', 'Waiting for your input', 'LIVE'],
    ['idle', 'Waiting for the next step', 'LIVE'],
    ['cancelled', 'Step cancelled', 'LIVE'],
  ] as const)('says %s as “%s” while the run is going (%s)', async (phase, label, mode) => {
    show(liveState({ phase, files: FILES }));

    const live = within(await panel());

    expect(await live.findByText(label)).toBeInTheDocument();
    expect(live.getByText(mode)).toBeInTheDocument();
  });

  it('says the workflow completed, and that it is a review, once it ended', async () => {
    show(liveState({ phase: 'ended', files: FILES }), { current: completed() });

    const live = within(await panel());

    expect(await live.findByText('Workflow completed')).toBeInTheDocument();
    expect(live.getByText('REVIEW')).toBeInTheDocument();
  });

  it('says the workflow stopped, with the work it left', async () => {
    const stopped = withCode(run(w, 'failed', {}), 'changes_available');
    show(liveState({ phase: 'stopped', files: FILES }), { current: stopped });

    const live = within(await panel());

    expect(await live.findByText('Workflow stopped')).toBeInTheDocument();
    expect(live.getByText('REVIEW')).toBeInTheDocument();
    expect(await files()).toBeTruthy();
  });

  it('says plainly when the worktree is unavailable or invalid', async () => {
    show(liveState({ availability: 'missing', phase: 'ended', files: FILES }), {
      current: completed(),
    });
    expect(await within(await panel()).findByText('Worktree unavailable')).toBeInTheDocument();
  });

  it('says the worktree is invalid when it is not what Atlas made', async () => {
    show(liveState({ availability: 'invalid', files: [] }));

    expect(await within(await panel()).findByText('Worktree invalid')).toBeInTheDocument();
    expect(screen.getByText(/not what Atlas made/)).toBeInTheDocument();
  });

  it('says it checks periodically when the platform gave no events', async () => {
    show(liveState({ observation: 'polling', files: FILES }));

    expect(await within(await panel()).findByText('Checking periodically')).toBeInTheDocument();
  });

  it('does not invent progress or an “editing …” line, and offers no way to apply the code', async () => {
    show(liveState({ files: FILES }));

    const live = within(await panel());
    await live.findByText('LIVE');

    expect(live.queryByRole('progressbar')).toBeNull();
    expect(live.queryByText(/editing/i)).toBeNull();
    expect(
      live.queryByRole('button', { name: /apply|merge|commit|push|discard|keep/i }),
    ).toBeNull();
  });
});

describe('the live workspace: events', () => {
  it('a file created, then modified, then removed shows up, changes and disappears', async () => {
    show(liveState({ files: FILES, revision: 1 }));
    await files();

    emit(liveUpdate(2, { changed: [fileChange('src/new.ts', 'added')], filesChanged: 3 }));
    expect(
      await (await files()).findByRole('button', { name: 'src/new.ts (Added)' }),
    ).toBeInTheDocument();

    emit(
      liveUpdate(3, {
        changed: [fileChange('src/new.ts', 'modified', { additions: 40 })],
        filesChanged: 3,
        additions: 46,
      }),
    );
    expect(
      await (await files()).findByRole('button', { name: 'src/new.ts (Modified)' }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText('Totals against the baseline')).toHaveTextContent('+46');

    emit(liveUpdate(4, { removed: ['src/new.ts'], filesChanged: 2 }));
    const tree = await files();
    await waitFor(() => {
      expect(tree.queryByRole('button', { name: /src\/new\.ts/ })).toBeNull();
    });
  });

  it('a deleted file is shown as removed, a rename as one file that remembers its old name', async () => {
    show(liveState({ files: FILES, revision: 1 }));
    await files();

    emit(
      liveUpdate(2, {
        full: true,
        changed: [
          fileChange('src/a.ts', 'deleted'),
          fileChange('lib/b.ts', 'renamed', { oldPath: 'src/b.ts' }),
        ],
        filesChanged: 2,
      }),
    );

    const tree = await files();
    expect(await tree.findByRole('button', { name: 'src/a.ts (Removed)' })).toBeInTheDocument();
    const renamed = tree.getByRole('button', { name: 'lib/b.ts (Renamed)' });
    expect(renamed).toHaveAttribute('title', 'Renamed from src/b.ts');
    expect(tree.queryByRole('button', { name: /^src\/b\.ts/ })).toBeNull();
  });

  it('lists in the activity only what really happened, with the time it was reported', async () => {
    show(liveState({ files: FILES, revision: 1 }));
    await files();

    emit(
      liveUpdate(2, {
        changed: [fileChange('src/a.ts')],
        phase: 'waiting_for_input',
        updatedAt: Date.UTC(2026, 9, 5, 10, 32, 17),
      }),
    );

    const activity = await screen.findByRole('list', { name: 'Activity' });
    expect(within(activity).getByText('A step is waiting for input')).toBeInTheDocument();
    expect(within(activity).getByRole('button', { name: 'src/a.ts' })).toBeInTheDocument();
    expect(within(activity).getByText('modified')).toBeInTheDocument();
    expect(activity.querySelectorAll('li')).toHaveLength(2);
  });

  it('ignores an update older than the state it has', async () => {
    show(liveState({ files: FILES, revision: 5 }));
    await files();

    emit(liveUpdate(4, { removed: ['src/a.ts'], filesChanged: 1 }));
    emit(liveUpdate(5, { removed: ['src/a.ts'], filesChanged: 1 }));

    expect(
      (await files()).getByRole('button', { name: 'src/a.ts (Modified)' }),
    ).toBeInTheDocument();
    expect(getLiveWorkspace).toHaveBeenCalledTimes(1);
  });

  it('reads the snapshot again when a revision was skipped', async () => {
    show(liveState({ files: FILES, revision: 1 }));
    await files();
    vi.mocked(getLiveWorkspace).mockResolvedValue(
      liveState({ files: [fileChange('src/after-gap.ts')], revision: 9 }),
    );

    emit(liveUpdate(5, { changed: [fileChange('src/ignored.ts')] }));

    expect(
      await (await files()).findByRole('button', { name: 'src/after-gap.ts (Modified)' }),
    ).toBeInTheDocument();
    expect(getLiveWorkspace).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole('button', { name: /ignored/ })).toBeNull();
  });

  it('does not lose an update that arrives while the first snapshot is being read', async () => {
    let answer: (state: ReturnType<typeof liveState>) => void = () => undefined;
    show(null, {
      snapshot: () =>
        new Promise((resolve) => {
          answer = resolve;
        }),
    });
    await waitFor(() => {
      expect(subscribeToLiveUpdates).toHaveBeenCalled();
    });

    emit(liveUpdate(2, { changed: [fileChange('src/early.ts', 'added')], filesChanged: 2 }));
    emit(liveUpdate(1, { changed: [fileChange('src/stale.ts', 'added')] }));
    answer(liveState({ files: [fileChange('src/a.ts')], revision: 1 }));

    const tree = await files();
    expect(await tree.findByRole('button', { name: 'src/early.ts (Added)' })).toBeInTheDocument();
    expect(tree.queryByRole('button', { name: /stale/ })).toBeNull();
  });

  it('ignores the events of another run, and stops listening when it goes away', async () => {
    const { unmount } = show(liveState({ files: FILES, revision: 1 }));
    await files();

    emit(liveUpdate(2, { runId: 'wfx-other', changed: [fileChange('other.ts')] }));
    expect(screen.queryByRole('button', { name: /other\.ts/ })).toBeNull();

    unmount();
    expect(unlisten).toHaveBeenCalled();
  });

  it('never asks for the worktree again by itself: the backend says when something changed', async () => {
    show(liveState({ files: FILES }));
    await files();

    await new Promise((resolve) => setTimeout(resolve, 400));

    expect(getLiveWorkspace).toHaveBeenCalledTimes(1);
    expect(refreshLiveWorkspace).not.toHaveBeenCalled();
  });

  it('refreshes when asked to', async () => {
    const user = userEvent.setup();
    show(liveState({ files: FILES, revision: 1 }));
    await files();
    vi.mocked(refreshLiveWorkspace).mockResolvedValue(
      liveState({ files: [fileChange('src/refreshed.ts')], revision: 2 }),
    );

    await user.click(screen.getByRole('button', { name: 'Refresh' }));

    expect(
      await (await files()).findByRole('button', { name: 'src/refreshed.ts (Modified)' }),
    ).toBeInTheDocument();
  });
});

describe('the live workspace: waiting for a person', () => {
  it('stays on screen and keeps receiving events while an agent waits, and after the answer', async () => {
    const waiting = withCode(run(w, 'running', {}), 'in_progress');
    const view = show(liveState({ phase: 'waiting_for_input', files: FILES, revision: 1 }), {
      current: waiting,
    });
    const live = within(await panel());
    expect(await live.findByText('Waiting for your input')).toBeInTheDocument();
    expect(live.getByText('LIVE')).toBeInTheDocument();

    // Still reviewing while it waits: a file changes.
    emit(
      liveUpdate(2, {
        changed: [fileChange('src/while-waiting.ts', 'added')],
        phase: 'waiting_for_input',
        filesChanged: 3,
      }),
    );
    expect(
      await live.findByRole('button', { name: 'src/while-waiting.ts (Added)' }),
    ).toBeInTheDocument();

    // The person answers: the run goes on, the panel is the same one and nothing was reloaded.
    view.rerunWith(withCode(run(w, 'running', {}), 'in_progress'));
    emit(liveUpdate(3, { phase: 'running', filesChanged: 3 }));

    expect(await live.findByText('Agent working')).toBeInTheDocument();
    expect(live.getByRole('button', { name: 'src/while-waiting.ts (Added)' })).toBeInTheDocument();
    expect(getLiveWorkspace).toHaveBeenCalledTimes(1);

    // And the end.
    view.rerunWith(completed());
    emit(liveUpdate(4, { phase: 'ended', filesChanged: 3 }));
    expect(await live.findByText('Workflow completed')).toBeInTheDocument();
    expect(live.getByText('REVIEW')).toBeInTheDocument();
  });
});

describe('the live workspace: the viewer', () => {
  it('shows the real diff of the whole worktree until a file is chosen, then of that file', async () => {
    const user = userEvent.setup();
    show(liveState({ files: FILES }));

    const diff = await screen.findByLabelText('Diff against the baseline');
    expect(getLiveDiff).toHaveBeenCalledWith('wfx-1', undefined);
    expect(within(diff).getByText('new line')).toBeInTheDocument();
    expect(within(diff).getByText('old line')).toBeInTheDocument();

    await user.click((await files()).getByRole('button', { name: 'src/a.ts (Modified)' }));

    await waitFor(() => {
      expect(getLiveDiff).toHaveBeenCalledWith('wfx-1', 'src/a.ts');
    });
  });

  it('shows the current content of the chosen file, with line numbers, from the worktree', async () => {
    const user = userEvent.setup();
    show(liveState({ files: FILES }));
    await user.click((await files()).getByRole('button', { name: 'src/a.ts (Modified)' }));

    await user.click(screen.getByRole('tab', { name: 'File' }));

    const code = await screen.findByLabelText('Content of src/a.ts');
    expect(getLiveFile).toHaveBeenCalledWith('wfx-1', 'src/a.ts');
    expect(code).toHaveTextContent('1keep');
    expect(code).toHaveTextContent('2new line');
  });

  it('reads the selected file again when that file changes, and only then', async () => {
    const user = userEvent.setup();
    show(liveState({ files: FILES, revision: 1 }));
    await user.click((await files()).getByRole('button', { name: 'src/a.ts (Modified)' }));
    await user.click(screen.getByRole('tab', { name: 'File' }));
    await screen.findByLabelText('Content of src/a.ts');
    expect(getLiveFile).toHaveBeenCalledTimes(1);

    // Another file changes: nothing is read again.
    emit(liveUpdate(2, { changed: [fileChange('src/b.ts', 'added')] }));
    await new Promise((resolve) => setTimeout(resolve, 100));
    expect(getLiveFile).toHaveBeenCalledTimes(1);

    // The selected one changes, even with the same counts: it is read again.
    vi.mocked(getLiveFile).mockResolvedValue(text('keep\nnewer line\n'));
    emit(liveUpdate(3, { changed: [fileChange('src/a.ts')] }));

    await waitFor(() => {
      expect(screen.getByLabelText('Content of src/a.ts')).toHaveTextContent('newer line');
    });
    expect(getLiveFile).toHaveBeenCalledTimes(2);
  });

  it('keeps showing the previous text while the same file is read again', async () => {
    const user = userEvent.setup();
    show(liveState({ files: FILES, revision: 1 }));
    await user.click((await files()).getByRole('button', { name: 'src/a.ts (Modified)' }));
    await user.click(screen.getByRole('tab', { name: 'File' }));
    await screen.findByLabelText('Content of src/a.ts');
    vi.mocked(getLiveFile).mockReturnValue(new Promise(() => undefined));

    emit(liveUpdate(2, { changed: [fileChange('src/a.ts')] }));

    expect(screen.getByLabelText('Content of src/a.ts')).toHaveTextContent('new line');
  });

  it('does not read a deleted file: it says it was removed and keeps its diff', async () => {
    const user = userEvent.setup();
    show(liveState({ files: [fileChange('src/gone.ts', 'deleted')] }));
    await user.click((await files()).getByRole('button', { name: 'src/gone.ts (Removed)' }));

    await user.click(screen.getByRole('tab', { name: 'File' }));
    expect(screen.getByText(/This file was removed in the worktree/)).toBeInTheDocument();
    expect(getLiveFile).not.toHaveBeenCalled();

    await user.click(screen.getByRole('tab', { name: 'Diff' }));
    expect(await screen.findByLabelText('Diff against the baseline')).toBeInTheDocument();
    expect(getLiveDiff).toHaveBeenCalledWith('wfx-1', 'src/gone.ts');
  });

  it('says when a file is binary, or has no changes against the baseline', async () => {
    const user = userEvent.setup();
    show(liveState({ files: FILES }));
    vi.mocked(getLiveFile).mockResolvedValue({
      path: 'src/a.ts',
      kind: 'binary',
      content: null,
      size: 5,
      truncated: false,
    });
    await user.click((await files()).getByRole('button', { name: 'src/a.ts (Modified)' }));
    await user.click(screen.getByRole('tab', { name: 'File' }));
    expect(
      await screen.findByText('This is a binary file: its content is not shown.'),
    ).toBeInTheDocument();

    vi.mocked(getLiveDiff).mockResolvedValue('');
    emit(liveUpdate(2, { changed: [fileChange('src/a.ts')] }));
    await user.click(screen.getByRole('tab', { name: 'Diff' }));
    expect(await screen.findByText('No changes against the baseline.')).toBeInTheDocument();
  });

  it('shows the backend’s refusal of a path, and any other error, without breaking the panel', async () => {
    const user = userEvent.setup();
    show(liveState({ files: FILES }));
    vi.mocked(getLiveFile).mockRejectedValue(coded('worktree_invalid_state'));
    await user.click((await files()).getByRole('button', { name: 'src/a.ts (Modified)' }));

    await user.click(screen.getByRole('tab', { name: 'File' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'That worktree is not in a state where this can be done.',
    );
    expect((await files()).getByRole('button', { name: 'src/b.ts (Added)' })).toBeInTheDocument();
  });

  it('falls back to all changes when the selected file is no longer a change', async () => {
    const user = userEvent.setup();
    show(liveState({ files: FILES, revision: 1 }));
    await user.click((await files()).getByRole('button', { name: 'src/a.ts (Modified)' }));
    expect(screen.getByRole('button', { name: 'src/a.ts (Modified)' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );

    emit(liveUpdate(2, { removed: ['src/a.ts'], filesChanged: 1 }));

    const tree = await files();
    await waitFor(() => {
      expect(tree.queryByRole('button', { name: /src\/a\.ts/ })).toBeNull();
    });
    expect(screen.getByRole('button', { name: /All changes/ })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('after the worktree is gone, the diff comes from the saved changes and the file cannot be read', async () => {
    const user = userEvent.setup();
    show(liveState({ availability: 'missing', phase: 'ended', files: FILES }), {
      current: completed(),
    });
    await within(await panel()).findByText('Worktree unavailable');

    expect(getLiveDiff).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(getDiff).toHaveBeenCalledWith('wfx-1', undefined);
    });

    await user.click((await files()).getByRole('button', { name: 'src/a.ts (Modified)' }));
    await user.click(screen.getByRole('tab', { name: 'File' }));
    expect(screen.getByText(/no longer available/)).toBeInTheDocument();
    expect(getLiveFile).not.toHaveBeenCalled();
  });

  it('groups files by folder and closes a folder', async () => {
    const user = userEvent.setup();
    show(liveState({ files: [fileChange('src/auth/login.ts'), fileChange('src/auth/token.ts')] }));

    const tree = await files();
    expect(tree.getByRole('button', { name: 'src/auth/login.ts (Modified)' })).toBeInTheDocument();

    await user.click(tree.getByRole('button', { name: /^src\/auth\s*2$/ }));

    expect(tree.queryByRole('button', { name: 'src/auth/login.ts (Modified)' })).toBeNull();
  });
});
