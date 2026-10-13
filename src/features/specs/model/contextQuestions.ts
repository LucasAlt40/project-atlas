import type { ProjectRoleDto } from '@/lib/tauri/commands';

/** What kind of project the spec is for, as far as Atlas can tell. */
export interface ProjectProfile {
  /** Screens and what people see. */
  ui: boolean;
  /** Code that runs on a server and talks to other systems. */
  server: boolean;
  /** Owns data: tables, migrations, queries. */
  data: boolean;
}

type Topic = 'data' | 'integration' | 'auth' | 'ui' | 'privacy' | 'volume' | 'rules';

export type ContextQuestionId =
  | 'goal'
  | 'untouched'
  | 'existing'
  | 'data'
  | 'dataMigration'
  | 'integration'
  | 'contract'
  | 'auth'
  | 'ui'
  | 'privacy'
  | 'volume'
  | 'rules';

export interface ContextQuestion {
  id: ContextQuestionId;
  /** Why it is asked: the topic of the description that brought it, or `base`. */
  because: Topic | 'base';
}

const UI_TECH =
  /react|vue|angular|svelte|next|nuxt|vite|tailwind|html|css|flutter|swift|android|ionic/i;
const DATA_TECH =
  /prisma|typeorm|sequelize|sql|postgres|mysql|mongo|sqlite|django|rails|laravel|spring|hibernate|diesel|knex|drizzle|entity/i;
const SERVER_TECH =
  /express|nest|fastify|django|flask|fastapi|rails|laravel|spring|\bgo\b|rust|java|php|\.net|dotnet|node/i;

/**
 * What the project is. The role the person gave it decides; without one, the technologies Atlas
 * recognised in the folder do; with neither, nothing is ruled out (asking too much is better than
 * hiding the question that mattered).
 */
export function profileOf(role: ProjectRoleDto | null, technologies: string[]): ProjectProfile {
  switch (role) {
    case 'frontend':
      return { ui: true, server: false, data: false };
    case 'backend':
    case 'service':
      return { ui: false, server: true, data: true };
    case 'bff':
      return { ui: false, server: true, data: false };
    case 'library':
      return { ui: false, server: false, data: false };
    case 'infrastructure':
      return { ui: false, server: true, data: false };
    case 'other':
    case null:
      break;
  }
  const text = technologies.join(' ');
  const profile = {
    ui: UI_TECH.test(text),
    server: SERVER_TECH.test(text),
    data: DATA_TECH.test(text),
  };
  return profile.ui || profile.server || profile.data
    ? profile
    : { ui: true, server: true, data: true };
}

const TOPICS: [Topic, RegExp][] = [
  [
    'data',
    /banco|tabela|coluna|migra[çc]|registro|persist|salv|armazen|exclu|delet|apag|database|\btable|column|schema|\bsql|record|store|insert|update/i,
  ],
  [
    'integration',
    /\bapi\b|integra|gateway|pagamento|cobran|assinatura|webhook|servi[çc]o|terceiro|e-?mail|sms|fila|queue|payment|subscription|provider|third.?party|stripe|pagbank|endpoint/i,
  ],
  ['auth', /login|permiss|perfil|autentic|autoriz|\brole\b|admin|senha|token|sess[ãa]o|auth/i],
  [
    'ui',
    /tela|bot[ãa]o|p[áa]gina|layout|formul[áa]rio|modal|menu|screen|button|page|form\b|\bui\b|interface/i,
  ],
  [
    'privacy',
    /lgpd|gdpr|dados pessoais|privacidade|auditoria|\blog\b|logs|reten[çc]|personal data|privacy|audit|retention|exclus[ãa]o de conta|delete account/i,
  ],
  [
    'volume',
    /lote|volume|performance|desempenho|ass[ií]ncron|\bjob\b|cron|agendad|batch|schedule|scale|milh[õo]es/i,
  ],
  [
    'rules',
    /regra|limite|prazo|valor|pre[çc]o|taxa|cota|exce[çc][ãa]o|rule|limit|deadline|price|fee|quota/i,
  ],
];

/** What the description is about. */
export function topicsOf(request: string): Topic[] {
  return TOPICS.filter(([, pattern]) => pattern.test(request)).map(([topic]) => topic);
}

/**
 * The questions worth asking for this request in this project. Not a form to fill in: each one is
 * a prompt for context the agent cannot see, and the ones that do not apply are not asked (no
 * question about tables for a project that only draws screens).
 */
export function questionsFor(request: string, profile: ProjectProfile): ContextQuestion[] {
  const topics = new Set(topicsOf(request));
  const questions: ContextQuestion[] = [
    { id: 'goal', because: 'base' },
    { id: 'untouched', because: 'base' },
    { id: 'existing', because: 'base' },
  ];
  if (profile.data && topics.has('data')) {
    questions.push({ id: 'data', because: 'data' }, { id: 'dataMigration', because: 'data' });
  }
  if (topics.has('integration')) {
    questions.push({ id: profile.server ? 'integration' : 'contract', because: 'integration' });
  }
  if (topics.has('auth')) questions.push({ id: 'auth', because: 'auth' });
  if (profile.ui && topics.has('ui')) questions.push({ id: 'ui', because: 'ui' });
  if (topics.has('privacy')) questions.push({ id: 'privacy', because: 'privacy' });
  if (profile.server && topics.has('volume')) questions.push({ id: 'volume', because: 'volume' });
  if (topics.has('rules')) questions.push({ id: 'rules', because: 'rules' });
  return questions;
}

/** The answers, as context the agent reads: each under the question it answers. */
export function contextFromAnswers(
  extra: string,
  answered: { question: string; answer: string }[],
): string {
  const blocks = answered
    .filter((a) => a.answer.trim() !== '')
    .map((a) => `- ${a.question}\n  ${a.answer.trim().replace(/\n/g, '\n  ')}`);
  return [extra.trim(), blocks.length > 0 ? `Answers from the person:\n${blocks.join('\n')}` : '']
    .filter((part) => part !== '')
    .join('\n\n');
}
