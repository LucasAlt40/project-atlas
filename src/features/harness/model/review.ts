import type {
  ConflictDto,
  FindingCategoryDto,
  FindingDto,
  InitModeDto,
  InitializeInputDto,
  ProjectAnalysisDto,
  UserKnowledgeDto,
} from '@/lib/tauri/commands';

/** The findings that describe the stack, in the order the first screen shows them. */
export const STACK_CATEGORIES: readonly FindingCategoryDto[] = [
  'language',
  'framework',
  'runtime',
  'package_manager',
  'database',
  'infrastructure',
  'testing',
  'tooling',
  'ci',
];

/** The review's sections and what goes in each. */
export const REVIEW_GROUPS = [
  {
    id: 'stack',
    categories: [
      'language',
      'framework',
      'runtime',
      'package_manager',
      'database',
      'infrastructure',
      'tooling',
      'ci',
    ],
  },
  { id: 'dependencies', categories: ['dependency', 'data', 'integration'] },
  { id: 'testing', categories: ['testing', 'build'] },
  { id: 'entry', categories: ['entry_point'] },
  { id: 'architecture', categories: ['architecture', 'module'] },
  { id: 'conventions', categories: ['convention'] },
] as const satisfies readonly { id: string; categories: readonly FindingCategoryDto[] }[];

export type ReviewGroupId = (typeof REVIEW_GROUPS)[number]['id'];

const isStack = (finding: FindingDto) => STACK_CATEGORIES.includes(finding.category);

/** Stack findings in review order. */
export function stackFindings(analysis: ProjectAnalysisDto): FindingDto[] {
  return STACK_CATEGORIES.flatMap((category) =>
    analysis.findings.filter((finding) => finding.category === category),
  );
}

/** Findings that can be reviewed: Atlas's own "unknown" and environment notes are not. */
function reviewable(finding: FindingDto): boolean {
  return finding.origin !== 'generated' && finding.category !== 'environment';
}

/** The review's sections with their findings; empty sections are left out. */
export function reviewGroups(
  analysis: ProjectAnalysisDto,
): { id: ReviewGroupId; findings: FindingDto[] }[] {
  return REVIEW_GROUPS.map((group) => ({
    id: group.id,
    findings: analysis.findings.filter(
      (f) => reviewable(f) && (group.categories as readonly string[]).includes(f.category),
    ),
  })).filter((group) => group.findings.length > 0);
}

/** Architecture patterns that were actually recognised (`unknown` is not a pattern). */
export function architectureFindings(analysis: ProjectAnalysisDto): FindingDto[] {
  return analysis.findings.filter((f) => f.category === 'architecture' && reviewable(f));
}

export function repositoryFindings(analysis: ProjectAnalysisDto): FindingDto[] {
  return analysis.findings.filter(
    (f) => f.category === 'repository' || f.category === 'environment',
  );
}

/** Findings the first screen lists as detected. */
export function detectedStack(analysis: ProjectAnalysisDto): FindingDto[] {
  return analysis.findings.filter(isStack);
}

/** Only an inference is something the user can confirm. */
export function canConfirm(finding: FindingDto): boolean {
  return finding.origin === 'inference';
}

/** What the user has decided so far in the review. */
export interface ReviewState {
  /** Finding ids the user unticked. */
  excluded: string[];
  /** Inference ids the user confirmed. */
  confirmed: string[];
  /** Finding id -> the value the user typed or chose. */
  values: Record<string, string>;
  purpose: string;
  users: string;
  concepts: string;
  businessRules: string;
  constraints: string;
  decisions: string;
  /** Fields prefilled with a model's draft rather than the user's own words. */
  suggested: (keyof UserKnowledgeDto)[];
  /** Keep `.atlas/` out of Git (the project's `.gitignore`). On unless the user turns it off. */
  ignoreInGit: boolean;
}

const BUSINESS_FIELDS = [
  'purpose',
  'users',
  'concepts',
  'businessRules',
  'constraints',
  'decisions',
] as const;

/**
 * Where a review starts: last time's choices if there were any (and the user's own text as it is
 * in `.atlas/`), and otherwise everything ticked except low-confidence guesses, which the user
 * must opt into.
 */
export function initialReview(analysis: ProjectAnalysisDto): ReviewState {
  const hadChoices =
    analysis.previousExcluded.length > 0 || Object.keys(analysis.previousCorrections).length > 0;
  const weakGuesses = analysis.findings
    .filter((f) => f.origin === 'inference' && f.confidence === 'low')
    .map((f) => f.id);
  const resolutions = Object.fromEntries(
    analysis.conflicts.flatMap((c) => (c.resolution ? [[c.findingId, c.resolution]] : [])),
  );
  // A model's draft fills only what the user has not written, and is marked as a draft.
  const suggested = BUSINESS_FIELDS.filter(
    (field) => analysis.user[field] === '' && analysis.suggestedUser[field] !== '',
  );
  const user = { ...analysis.user };
  for (const field of suggested) user[field] = analysis.suggestedUser[field];
  return {
    excluded: hadChoices ? [...analysis.previousExcluded] : weakGuesses,
    confirmed: [...analysis.previousConfirmed],
    values: { ...resolutions, ...analysis.previousCorrections },
    ...user,
    suggested: [...suggested],
    ignoreInGit: true,
  };
}

/** The core's input: only real changes become corrections, and only known findings count. */
export function buildInput(
  analysis: ProjectAnalysisDto,
  state: ReviewState,
  mode: InitModeDto,
): InitializeInputDto {
  const known = new Map(analysis.findings.map((f) => [f.id, f]));
  const corrections: Record<string, string> = {};
  for (const [id, raw] of Object.entries(state.values)) {
    const value = raw.trim();
    const finding = known.get(id);
    if (finding && value !== '' && value !== finding.value) corrections[id] = value;
  }
  return {
    mode,
    excluded: state.excluded.filter((id) => known.has(id)),
    corrections,
    confirmed: state.confirmed.filter((id) => {
      const finding = known.get(id);
      return finding !== undefined && canConfirm(finding);
    }),
    purpose: state.purpose.trim(),
    users: state.users.trim(),
    concepts: state.concepts.trim(),
    businessRules: state.businessRules.trim(),
    constraints: state.constraints.trim(),
    decisions: state.decisions.trim(),
    ignoreInGit: state.ignoreInGit,
  };
}

/** `true` / `possible` / `unknown` are markers, not values a person would edit. */
export function isEditableValue(finding: FindingDto): boolean {
  return (
    !['true', 'possible', 'unknown'].includes(finding.value) &&
    (STACK_CATEGORIES as readonly string[]).includes(finding.category) &&
    finding.category !== 'testing'
  );
}

/** Conflicts the user has not decided yet (in this review or in an earlier one). */
export function unresolvedConflicts(analysis: ProjectAnalysisDto, state: ReviewState) {
  return analysis.conflicts.filter(
    (c: ConflictDto) => !(c.findingId in state.values) && c.resolution === null,
  );
}

const SOURCE_ROOTS = /^(?:.*?\/)?src\/(?:main|test)\/(?:java|kotlin|scala|groovy|resources)\//;
const DOMAIN_PREFIXES = new Set([
  'br',
  'com',
  'org',
  'net',
  'io',
  'dev',
  'app',
  'co',
  'gov',
  'edu',
]);

/**
 * A path as it is read at a glance: past the language's source root and the reversed-domain
 * prefix (`src/main/java/br/org/acme/infra/Foo.java` becomes `acme/infra/Foo.java`).
 */
export function shortPath(path: string): string {
  const rooted = SOURCE_ROOTS.test(path) ? path.replace(SOURCE_ROOTS, '') : path;
  if (rooted === path) return path;
  const parts = rooted.split('/');
  let start = 0;
  while (start < parts.length - 2 && DOMAIN_PREFIXES.has(parts[start] ?? '')) start += 1;
  return parts.slice(start).join('/');
}
