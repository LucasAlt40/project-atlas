//! A mandatory rule against text that tells the agent the opposite.
//!
//! Rules that share a `topic` are settled by resolution. Without one, Atlas used to leave two
//! opposite instructions side by side and let the model choose. This looks for the cases where the
//! opposition is plain, and only those: a small closed vocabulary of actions (tests, commit, push,
//! merge, destructive commands) and one path rule ("do not change anything outside `src/`"), each
//! in English and Portuguese, read as *do* or *do not* from the words around them.
//!
//! No meaning is inferred and nothing is learned. A line is a **directive** only when it names one
//! of those actions with a clear polarity (negation words are counted, so "do not skip tests" is
//! a *do*). The result is one of two things, and they are not mixed:
//!
//! - **proven**: both sides are plain and opposite. From a source that may instruct (the task, the
//!   agent's instructions, another rule) this is an `Error`: a person decides, and without one the
//!   step does not run. From a source that may only inform (Harness, skill, handoff) it is a
//!   `Warning`: that text cannot instruct, so it cannot override the rule, but it is shown.
//! - **possible**: the same action is touched in opposite directions but a side is hedged ("if
//!   needed"), or tangled in negations. A `Warning`, nothing more.
//!
//! What is not in the vocabulary is not reported: when a conflict cannot be shown, saying nothing
//! is more honest than guessing.

use std::collections::HashSet;

use super::rules::MandatoryRule;
use super::{excerpt, issue};
use crate::application::optimization::context::text::fenced_lines;
use crate::application::optimization::context::ContextItem;
use crate::domain::guardrail::{IssueCode, IssueSeverity, ReviewIssue, SourceTrust};
use crate::domain::optimization::SectionKind;

struct Action {
    id: &'static str,
    /// Words that name it (whole words, lowercase).
    nouns: &'static [&'static str],
    /// Phrases that name it (substrings, lowercase).
    phrases: &'static [&'static str],
    /// Words that make a mention an instruction to do it. Without one, a mention is not a directive
    /// (a report that tests failed does not tell anyone to run them).
    verbs: &'static [&'static str],
}

const ACTIONS: &[Action] = &[
    Action {
        id: "tests",
        nouns: &[
            "test", "tests", "testing", "teste", "testes", "testar", "testado",
        ],
        phrases: &[],
        verbs: &[
            "write",
            "run",
            "execute",
            "add",
            "create",
            "include",
            "perform",
            "have",
            "has",
            "need",
            "needs",
            "must",
            "should",
            "require",
            "required",
            "always",
            "ensure",
            "verify",
            "test",
            "escreva",
            "rode",
            "execute",
            "crie",
            "adicione",
            "inclua",
            "realize",
            "possuir",
            "ter",
            "deve",
            "devem",
            "precisa",
            "precisam",
            "sempre",
            "garanta",
            "verifique",
            "testar",
            "faça",
            "faca",
        ],
    },
    Action {
        id: "commit",
        nouns: &["commit", "commits", "commitar", "commite"],
        phrases: &[],
        verbs: &[
            "commit", "commits", "commitar", "commite", "make", "create", "git", "faça", "faca",
            "crie", "must", "deve", "always", "sempre",
        ],
    },
    Action {
        id: "push",
        nouns: &["push", "pushes", "pushar"],
        phrases: &[],
        verbs: &[
            "push", "pushes", "pushar", "git", "faça", "faca", "make", "must", "deve", "always",
            "sempre",
        ],
    },
    Action {
        id: "merge",
        nouns: &["merge", "merges", "mergear"],
        phrases: &[],
        verbs: &[
            "merge", "merges", "mergear", "git", "faça", "faca", "make", "must", "deve", "always",
            "sempre",
        ],
    },
    Action {
        id: "destructive commands",
        nouns: &[],
        phrases: &[
            "destructive command",
            "destructive operation",
            "comando destrutivo",
            "comandos destrutivos",
            "operação destrutiva",
            "operações destrutivas",
            "rm -rf",
            "drop table",
            "drop database",
            "force push",
            "force-push",
            "push --force",
            "push -f",
        ],
        verbs: &[
            "run", "execute", "use", "apply", "perform", "rode", "execute", "aplique", "realize",
            "faça", "faca", "rm", "drop", "must", "deve", "always", "sempre",
        ],
    },
    Action {
        id: "deploy",
        nouns: &[
            "deploy",
            "deploys",
            "deployment",
            "implantar",
            "implante",
            "implantação",
        ],
        phrases: &[],
        verbs: &[
            "deploy",
            "deploys",
            "implantar",
            "implante",
            "run",
            "execute",
            "perform",
            "faça",
            "faca",
            "realize",
            "must",
            "deve",
            "always",
            "sempre",
        ],
    },
    Action {
        id: "dependencies",
        nouns: &[
            "dependency",
            "dependencies",
            "dependência",
            "dependências",
            "dependencia",
            "dependencias",
        ],
        phrases: &[
            "npm install",
            "pip install",
            "cargo add",
            "yarn add",
            "pnpm add",
        ],
        verbs: &[
            "install", "add", "upgrade", "update", "instale", "adicione", "atualize", "use",
            "must", "deve", "always", "sempre",
        ],
    },
    Action {
        id: "deleting files",
        nouns: &[],
        phrases: &[
            "delete files",
            "delete any file",
            "remove files",
            "apague arquivos",
            "apagar arquivos",
            "remova arquivos",
            "exclua arquivos",
        ],
        verbs: &[
            "delete", "remove", "apague", "apagar", "remova", "exclua", "must", "deve", "always",
            "sempre",
        ],
    },
];

/// Words that turn a mention into a "do not" (counted: two cancel each other out).
const NEGATIONS: &[&str] = &[
    "not",
    "never",
    "without",
    "skip",
    "ignore",
    "omit",
    "avoid",
    "don't",
    "dont",
    "can't",
    "cannot",
    "não",
    "nao",
    "nunca",
    "sem",
    "pule",
    "pular",
    "ignorar",
    "omita",
    "evite",
    "nem",
    "skipped",
    "skipping",
    "skips",
    "ignored",
    "ignoring",
    "omitted",
    "avoiding",
    "pulados",
    "pulado",
    "pulada",
    "pulou",
    "pulem",
    "ignorados",
    "ignorado",
    "ignorem",
    "omitidos",
    "omitam",
    "evitar",
    "evitem",
];

/// Words that undo a negation next to them ("never forget tests").
const REVERSALS: &[&str] = &["forget", "esqueça", "esqueca", "esquecer"];

/// Words that say the instruction is soft or conditional: the conflict is possible, not proven.
const HEDGES: &[&str] = &[
    "if possible",
    "if needed",
    "if necessary",
    "when possible",
    "maybe",
    "perhaps",
    "can be",
    "may be",
    "optional",
    "se possível",
    "se possivel",
    "se necessário",
    "se necessario",
    "quando possível",
    "talvez",
    "podem ser",
    "pode ser",
    "opcional",
];

/// A line that asks, reports or explains is not an instruction, even when it names an action and a
/// negation ("why did we not commit yesterday?"): the conflict, if any, is only possible.
const NOT_AN_INSTRUCTION: &[&str] = &[
    "?",
    "why",
    "por que",
    "porque",
    "por quê",
    "explain",
    "explique",
    "describe",
    "descreva",
    "did not",
    "didn't",
    "wasn't",
    "weren't",
    "hasn't",
    "haven't",
    "we did",
    "não fizemos",
    "nao fizemos",
    "não fiz ",
    "não foi",
    "nao foi",
    "fizemos",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Polarity {
    Do,
    DoNot,
}

#[derive(Debug, Clone)]
struct Atom {
    action: &'static str,
    polarity: Polarity,
    /// Plain: at most two negation words and no hedge.
    clean: bool,
}

fn tokens(lower: &str) -> Vec<String> {
    lower
        .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '’' || c == '-'))
        .filter(|t| !t.is_empty())
        .map(|t| t.replace('’', "'"))
        .collect()
}

/// A bullet or tag in front of a line is not part of what it says.
fn body(line: &str) -> &str {
    let line = line.trim().trim_start_matches(['-', '*', ' ']);
    match line.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
        Some((_, rest)) => rest.trim(),
        None => line,
    }
}

/// Negation words in a line. "no" is one only in front of an action ("no tests"): in Portuguese
/// it is "at the" ("faça push no final").
fn negations_in(words: &[String]) -> usize {
    let heads = |w: &str| {
        ACTIONS.iter().any(|a| a.nouns.contains(&w))
            || matches!(
                w,
                "destructive" | "destrutivo" | "destrutivos" | "rm" | "drop"
            )
    };
    words
        .iter()
        .enumerate()
        .filter(|(i, w)| {
            NEGATIONS.contains(&w.as_str())
                || REVERSALS.contains(&w.as_str())
                || (w.as_str() == "no" && words.get(i + 1).is_some_and(|next| heads(next)))
        })
        .count()
}

/// Where, in tokens, the line first names the action.
fn mention_at(action: &Action, lower: &str, words: &[String]) -> Option<usize> {
    let by_word = words
        .iter()
        .position(|w| action.nouns.contains(&w.as_str()));
    let by_phrase = action
        .phrases
        .iter()
        .filter_map(|p| lower.find(p))
        .min()
        .map(|at| tokens(&lower[..at]).len());
    match (by_word, by_phrase) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// A negation sits right before the action ("do not write tests", "sem testes"), not somewhere
/// else in the sentence.
fn negation_is_near(words: &[String], mention: usize) -> bool {
    words
        .iter()
        .enumerate()
        .take(mention)
        .skip(mention.saturating_sub(4))
        .any(|(i, w)| {
            NEGATIONS.contains(&w.as_str())
                || REVERSALS.contains(&w.as_str())
                || (w.as_str() == "no" && i + 1 == mention)
        })
}

/// The action appears inside quotes or backticks: it is being talked about, not asked for.
fn mentioned_in_quotes(line: &str, action: &Action) -> bool {
    let lower = line.to_lowercase();
    let mut inside = false;
    let mut segment = String::new();
    let mut quoted: Vec<String> = Vec::new();
    for c in lower.chars() {
        if matches!(c, '"' | '“' | '”' | '`') {
            if inside {
                quoted.push(std::mem::take(&mut segment));
            }
            inside = !inside;
        } else if inside {
            segment.push(c);
        }
    }
    quoted.iter().any(|q| {
        let words = tokens(q);
        action.nouns.iter().any(|n| words.iter().any(|w| w == n))
            || action.phrases.iter().any(|p| q.contains(p))
    })
}

fn atoms_of(line: &str) -> Vec<Atom> {
    let lower = body(line).to_lowercase();
    let words = tokens(&lower);
    let negations = negations_in(&words);
    let hedged = HEDGES.iter().any(|h| lower.contains(h));
    let asks_or_reports = NOT_AN_INSTRUCTION.iter().any(|m| lower.contains(m));
    let mut found = Vec::new();
    for action in ACTIONS {
        let named = action.nouns.iter().any(|n| words.iter().any(|w| w == n))
            || action.phrases.iter().any(|p| lower.contains(p));
        if !named {
            continue;
        }
        let polarity = if negations % 2 == 1 {
            Polarity::DoNot
        } else {
            Polarity::Do
        };
        // A plain "do" needs a verb of doing; "do not" needs only the negation, and so does a
        // double negation ("do not skip the tests"), which is an instruction in itself.
        if polarity == Polarity::Do
            && negations == 0
            && !action.verbs.iter().any(|v| words.iter().any(|w| w == v))
        {
            continue;
        }
        found.push(Atom {
            action: action.id,
            polarity,
            clean: negations <= 2
                && !hedged
                && !asks_or_reports
                && !mentioned_in_quotes(body(line), action)
                && (negations == 0
                    || mention_at(action, &lower, &words)
                        .is_some_and(|at| negation_is_near(&words, at))),
        });
    }
    found
}

/// "Do not change anything outside src/": the directory that is allowed.
fn allowed_dir(line: &str) -> Option<String> {
    let lower = body(line).to_lowercase();
    let words = tokens(&lower);
    let forbids = words.iter().any(|w| {
        matches!(
            w.as_str(),
            "not" | "never" | "não" | "nao" | "nunca" | "don't" | "dont"
        )
    });
    let changes = words.iter().any(|w| {
        matches!(
            w.as_str(),
            "change"
                | "edit"
                | "modify"
                | "alter"
                | "write"
                | "touch"
                | "altere"
                | "edite"
                | "modifique"
                | "mude"
                | "escreva"
                | "alterar"
                | "editar"
                | "modificar"
        )
    });
    let outside = lower
        .find("outside of ")
        .map(|i| (i, "outside of ".len()))
        .or_else(|| lower.find("outside ").map(|i| (i, "outside ".len())))
        .or_else(|| lower.find("fora de ").map(|i| (i, "fora de ".len())))
        .or_else(|| lower.find("fora do ").map(|i| (i, "fora do ".len())))
        .or_else(|| lower.find("fora da ").map(|i| (i, "fora da ".len())));
    if !(forbids && changes) {
        return None;
    }
    let (at, len) = outside?;
    let dir = lower[at + len..]
        .split_whitespace()
        .next()?
        .trim_matches(|c: char| matches!(c, '`' | '"' | '\'' | '.' | ',' | ';' | ':'));
    (!dir.is_empty() && !dir.contains(char::is_whitespace)).then(|| normalize(dir))
}

fn normalize(path: &str) -> String {
    let path = path.trim_start_matches("./");
    if path.ends_with('/') {
        path.to_owned()
    } else {
        format!("{path}/")
    }
}

/// Files a plain instruction tells the agent to change ("edit package.json").
fn edited_paths(line: &str) -> Vec<String> {
    let lower = body(line).to_lowercase();
    let words = tokens(&lower);
    let negated = negations_in(&words) > 0;
    let edits = words.iter().any(|w| {
        matches!(
            w.as_str(),
            "edit"
                | "modify"
                | "change"
                | "update"
                | "alter"
                | "edite"
                | "altere"
                | "modifique"
                | "mude"
                | "atualize"
        )
    });
    if negated || !edits {
        return Vec::new();
    }
    body(line)
        .split_whitespace()
        .map(|w| {
            let punctuation =
                |c: char| matches!(c, '`' | '"' | '\'' | ',' | ';' | ':' | '(' | ')' | '.');
            // A sentence's full stop is not part of a name; a dot inside it is.
            w.trim_matches(punctuation)
        })
        .filter(|w| {
            !w.is_empty()
                && !w.contains("://")
                && (w.contains('/')
                    || w.rsplit_once('.').is_some_and(|(stem, ext)| {
                        !stem.is_empty()
                            && (1..=5).contains(&ext.len())
                            && ext.chars().all(char::is_alphanumeric)
                    }))
        })
        .map(|w| w.trim_start_matches("./").to_lowercase())
        .collect()
}

/// What a source says, line by line, outside code.
fn lines_of(item: &ContextItem) -> Vec<&str> {
    let fenced = fenced_lines(&item.content);
    item.content
        .lines()
        .zip(fenced)
        .filter(|(line, in_code)| !in_code && !line.trim().is_empty())
        .map(|(line, _)| line)
        .collect()
}

enum Finding {
    Proven,
    Possible,
}

pub fn find(items: &[ContextItem], mandatory: &[MandatoryRule]) -> Vec<ReviewIssue> {
    let mut issues = Vec::new();
    let mut reported: HashSet<(String, String, String)> = HashSet::new();
    for rule in mandatory {
        let own = format!("rule:{}", rule.reference);
        let rule_atoms: Vec<Atom> = rule.content.lines().flat_map(atoms_of).collect();
        let rule_bans: Vec<String> = rule.content.lines().filter_map(allowed_dir).collect();
        for item in items {
            // Atlas's own text is the hierarchy, not a party to it; a rule is not its own opponent.
            if item.id == own || SourceTrust::of(item.source) == SourceTrust::Atlas {
                continue;
            }
            // May this text instruct? Only text that may can contradict a mandatory rule for real.
            let instructs = item.authority.may_instruct();
            for line in lines_of(item) {
                let mut hits: Vec<(String, Finding)> = Vec::new();
                for other in atoms_of(line) {
                    for mine in rule_atoms.iter().filter(|m| m.action == other.action) {
                        if mine.polarity == other.polarity {
                            continue;
                        }
                        let finding = if mine.clean && other.clean {
                            Finding::Proven
                        } else {
                            Finding::Possible
                        };
                        hits.push((other.action.to_owned(), finding));
                    }
                }
                for allowed in &rule_bans {
                    if edited_paths(line)
                        .iter()
                        .any(|p| !p.starts_with(allowed.as_str()))
                    {
                        hits.push((format!("changes outside {allowed}"), Finding::Proven));
                    }
                }
                for (what, finding) in hits {
                    if !reported.insert((rule.reference.clone(), item.id.clone(), what.clone())) {
                        continue;
                    }
                    issues.push(report(rule, item, line, &what, &finding, instructs));
                }
            }
        }
    }
    issues
}

fn report(
    rule: &MandatoryRule,
    item: &ContextItem,
    line: &str,
    what: &str,
    finding: &Finding,
    instructs: bool,
) -> ReviewIssue {
    let (code, severity, message) = match (finding, instructs) {
        (Finding::Proven, true) => (
            IssueCode::RuleConflict,
            IssueSeverity::Error,
            format!(
                "{} tells the agent the opposite of the mandatory rule {} about {what}",
                item.id, rule.reference
            ),
        ),
        (Finding::Proven, false) => (
            IssueCode::RuleConflict,
            IssueSeverity::Warning,
            format!(
                "{} (text that cannot instruct) says the opposite of the mandatory rule {} about {what}; the rule governs",
                item.id, rule.reference
            ),
        ),
        (Finding::Possible, _) => (
            IssueCode::PossibleRuleConflict,
            IssueSeverity::Warning,
            format!(
                "{} may go against the mandatory rule {} about {what}, but not plainly enough to say",
                item.id, rule.reference
            ),
        ),
    };
    let mut found = issue(code, severity, item.source, message);
    found.other_source = Some(SectionKind::Rules);
    found.excerpt = excerpt(line);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(line: &str) -> Vec<(&'static str, Polarity, bool)> {
        atoms_of(line)
            .iter()
            .map(|a| (a.action, a.polarity, a.clean))
            .collect()
    }

    #[test]
    fn polarity_is_read_from_the_negations_around_the_action() {
        use Polarity::{Do, DoNot};
        assert_eq!(
            atom("Todo código deve possuir testes."),
            [("tests", Do, true)]
        );
        assert_eq!(
            atom("Não escreva testes para esta tarefa."),
            [("tests", DoNot, true)]
        );
        assert_eq!(atom("Sempre execute testes"), [("tests", Do, true)]);
        assert_eq!(atom("do not run tests"), [("tests", DoNot, true)]);
        // Two negations make a "do".
        assert_eq!(atom("Do not skip the tests"), [("tests", Do, true)]);
        assert_eq!(atom("nunca pule os testes"), [("tests", Do, true)]);
        assert_eq!(atom("Never forget to run the tests"), [("tests", Do, true)]);
        assert_eq!(atom("No tests for this fix"), [("tests", DoNot, true)]);
        // "no" is "at the" in Portuguese.
        assert_eq!(atom("Faça push no final"), [("push", Do, true)]);
        assert_eq!(atom("Não faça commit"), [("commit", DoNot, true)]);
        assert_eq!(atom("Faça commit ao terminar"), [("commit", Do, true)]);
        assert_eq!(
            atom("Não execute comandos destrutivos"),
            [("destructive commands", DoNot, true)]
        );
    }

    #[test]
    fn a_mention_with_no_instruction_in_it_is_not_a_directive() {
        assert_eq!(atom("The tests failed yesterday"), []);
        assert_eq!(atom("Os testes estão lentos"), []);
        assert_eq!(atom("Bom dia"), []);
    }

    #[test]
    fn a_hedged_or_tangled_line_is_not_plain() {
        assert!(!atom("Skip tests if needed")[0].2);
        assert!(!atom("testes podem ser pulados se necessário")[0].2);
        assert!(!atom("do not, never, no tests")[0].2);
    }

    #[test]
    fn the_allowed_directory_is_read_from_the_prohibition() {
        assert_eq!(
            allowed_dir("Não altere arquivos fora de src/").as_deref(),
            Some("src/")
        );
        assert_eq!(
            allowed_dir("Never modify anything outside `app`").as_deref(),
            Some("app/")
        );
        assert_eq!(allowed_dir("Edit files in src/"), None);
        assert_eq!(allowed_dir("Do not touch the config"), None);
    }

    #[test]
    fn files_to_change_are_read_only_from_a_plain_instruction() {
        assert_eq!(edited_paths("Edite package.json"), ["package.json"]);
        assert_eq!(edited_paths("Please edit `src/app.ts`."), ["src/app.ts"]);
        assert_eq!(edited_paths("Do not edit package.json"), [] as [String; 0]);
        assert_eq!(edited_paths("Read package.json"), [] as [String; 0]);
        assert_eq!(
            edited_paths("Edit https://example.test/x.json"),
            [] as [String; 0]
        );
    }
}
