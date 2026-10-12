import { useT } from '@/i18n/I18nProvider';
import { useProjectsOverview } from '../hooks/useProjectsOverview';
import type { Workspace } from '../types';
import projectStyles from './Projects.module.css';

/**
 * Each project of a multi-project workspace with the state of its Harness, in one line. The
 * place to act on them is the projects panel: this only says where things stand.
 */
export function ProjectsSummary({ workspace }: { workspace: Workspace }) {
  const t = useT();
  const signature = workspace.projects.map((p) => `${p.id}|${p.path}|${p.name}`).join(';');
  const { state } = useProjectsOverview(workspace.id, signature);
  const stateOf = (id: string) =>
    state.status === 'ready'
      ? state.overview.projects.find((p) => p.projectId === id)?.state
      : undefined;
  return (
    <ul className={projectStyles.checks} aria-label={t('projects.title', { name: workspace.name })}>
      {workspace.projects.map((project) => {
        const harness = stateOf(project.id);
        return (
          <li key={project.id} className={projectStyles.check}>
            <span className={projectStyles.name}>{project.name}</span>
            {project.role && (
              <span className={projectStyles.tag}>{t(`projects.role.${project.role}`)}</span>
            )}
            {harness && (
              <span className={projectStyles.state} data-state={harness}>
                {t(`projects.state.${harness}`)}
              </span>
            )}
          </li>
        );
      })}
    </ul>
  );
}
