use crate::lint::{line_col, LintDiagnostic};
use crate::syntax::token::{Token, TokenType};
use std::ops::Range;

pub(super) const IGNORE: &str = "rq-lint-ignore";
pub(super) const IGNORE_FILE: &str = "rq-lint-ignore-file";
pub(crate) const INVALID_RULE: &str = "invalid_lint_suppression";
pub(crate) const UNUSED_RULE: &str = "unused_lint_suppression";

enum Scope {
    File,
    Statement(Option<Range<usize>>),
    Line,
}

#[derive(Clone, Copy)]
enum Coverage {
    File,
    Span(Option<(Position, Position)>),
    Line(usize),
}

struct Directive {
    keyword: &'static str,
    offset: usize,
    scope: Scope,
    rules: Vec<(usize, String)>,
    on_own_line: bool,
}

struct Suppression {
    rule: String,
    offset: usize,
    coverage: Coverage,
    used: bool,
}

type Position = (usize, usize);

pub(super) struct ParsedComment {
    pub(super) keyword: &'static str,
    pub(super) rules: Vec<(usize, String)>,
}

pub fn apply(
    source: &str,
    display_path: &str,
    known_rules: &[&str],
    diagnostics: Vec<LintDiagnostic>,
) -> Vec<LintDiagnostic> {
    let Ok(tokens) = crate::syntax::tokenize(source) else {
        return diagnostics;
    };
    let directives = directives(&tokens);
    let mut meta = Vec::new();
    let mut suppressions = Vec::new();
    for directive in directives {
        collect_directive(
            source,
            display_path,
            known_rules,
            directive,
            &mut suppressions,
            &mut meta,
        );
    }
    let mut kept: Vec<LintDiagnostic> = diagnostics
        .into_iter()
        .filter(|diagnostic| !is_suppressed(diagnostic, &mut suppressions))
        .collect();
    kept.extend(meta);
    kept.extend(
        suppressions
            .iter()
            .filter(|s| !s.used)
            .map(|s| unused_diagnostic(source, display_path, s)),
    );
    kept
}

fn collect_directive(
    source: &str,
    display_path: &str,
    known_rules: &[&str],
    directive: Directive,
    suppressions: &mut Vec<Suppression>,
    meta: &mut Vec<LintDiagnostic>,
) {
    let keyword = directive.keyword;
    if keyword == IGNORE_FILE && !directive.on_own_line {
        meta.push(invalid_diagnostic(
            source,
            display_path,
            directive.offset,
            format!("`{keyword}` must be on its own line."),
        ));
        return;
    }
    if directive.rules.is_empty() {
        meta.push(invalid_diagnostic(
            source,
            display_path,
            directive.offset,
            format!(
                "`{keyword}` names no rule. List the rule ids to suppress, e.g. \
                 `// {keyword} hardcoded_secret: test fixture`."
            ),
        ));
        return;
    }
    let coverage = match &directive.scope {
        Scope::File => Coverage::File,
        Scope::Statement(range) => Coverage::Span(
            range
                .as_ref()
                .map(|r| (line_col(source, r.start), line_col(source, r.end))),
        ),
        Scope::Line => Coverage::Line(line_col(source, directive.offset).0),
    };
    for (offset, rule) in directive.rules {
        if !known_rules.contains(&rule.as_str()) {
            meta.push(invalid_diagnostic(
                source,
                display_path,
                offset,
                format!("`{rule}` is not a lint rule that can be suppressed."),
            ));
            continue;
        }
        suppressions.push(Suppression {
            rule,
            offset,
            coverage,
            used: false,
        });
    }
}

fn is_suppressed(diagnostic: &LintDiagnostic, suppressions: &mut [Suppression]) -> bool {
    let position = (diagnostic.line, diagnostic.column);
    let mut suppressed = false;
    for suppression in suppressions.iter_mut() {
        if suppression.rule != diagnostic.rule {
            continue;
        }
        let covers = match suppression.coverage {
            Coverage::File => true,
            Coverage::Span(span) => {
                span.is_some_and(|(start, end)| start <= position && position < end)
            }
            Coverage::Line(line) => line == diagnostic.line,
        };
        if covers {
            suppression.used = true;
            suppressed = true;
        }
    }
    suppressed
}

fn directives(tokens: &[Token]) -> Vec<Directive> {
    tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| token.token_type == TokenType::Comment)
        .filter_map(|(index, token)| {
            let parsed = parse_comment(&token.value)?;
            let keyword = parsed.keyword;
            let on_own_line = starts_line(tokens, index);
            let scope = match (keyword, on_own_line) {
                (IGNORE_FILE, _) => Scope::File,
                (_, true) => Scope::Statement(statement_range(tokens, index + 1)),
                (_, false) => Scope::Line,
            };
            Some(Directive {
                keyword,
                offset: token.span.start,
                scope,
                rules: parsed
                    .rules
                    .into_iter()
                    .map(|(offset, rule)| (token.span.start + offset, rule))
                    .collect(),
                on_own_line,
            })
        })
        .collect()
}

pub(super) fn parse_comment(comment: &str) -> Option<ParsedComment> {
    let body = comment.strip_prefix("//")?;
    let body_start = comment.len() - body.len();
    let trimmed = body.trim_start();
    let directive_start = body_start + body.len() - trimmed.len();
    let (keyword, rest) = [IGNORE_FILE, IGNORE]
        .into_iter()
        .find_map(|k| trimmed.strip_prefix(k).map(|rest| (k, rest)))?;
    if !rest.is_empty() && !rest.starts_with(|c: char| c == ':' || c.is_whitespace()) {
        return None;
    }
    let rule_list = rest.split_once(':').map_or(rest, |(rules, _)| rules);
    let keyword_end = directive_start + keyword.len();
    Some(ParsedComment {
        keyword,
        rules: rule_ids(rule_list, keyword_end),
    })
}

fn rule_ids(list: &str, base: usize) -> Vec<(usize, String)> {
    let mut ids = Vec::new();
    let mut current: Option<(usize, String)> = None;
    for (index, ch) in list.char_indices() {
        if ch == ',' || ch.is_whitespace() {
            ids.extend(current.take());
            continue;
        }
        current
            .get_or_insert_with(|| (base + index, String::new()))
            .1
            .push(ch);
    }
    ids.extend(current);
    ids
}

pub(super) fn starts_line(tokens: &[Token], index: usize) -> bool {
    tokens
        .iter()
        .take(index)
        .rev()
        .find(|t| t.token_type != TokenType::Whitespace)
        .is_none_or(|t| t.token_type == TokenType::Newline)
}

pub(super) fn statement_range(tokens: &[Token], from: usize) -> Option<Range<usize>> {
    let mut significant = tokens.get(from..)?.iter().filter(|t| {
        !matches!(
            t.token_type,
            TokenType::Whitespace | TokenType::Newline | TokenType::Comment
        )
    });
    let first = significant.next()?;
    let mut depth = 0i32;
    let mut end = first.span.start;
    for token in std::iter::once(first).chain(significant) {
        match token.value.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                depth -= 1;
                if depth < 0 {
                    break;
                }
                end = token.span.end;
                if depth == 0 && token.value == "}" {
                    break;
                }
                continue;
            }
            ";" | "," if depth == 0 => {
                end = token.span.end;
                break;
            }
            _ => {}
        }
        end = token.span.end;
    }
    Some(first.span.start..end)
}

fn invalid_diagnostic(
    source: &str,
    display_path: &str,
    offset: usize,
    message: String,
) -> LintDiagnostic {
    let (line, column) = line_col(source, offset);
    LintDiagnostic {
        severity: "error",
        rule: INVALID_RULE,
        message,
        line,
        column,
        file: Some(display_path.to_string()),
        suggested_fix: None,
    }
}

fn unused_diagnostic(
    source: &str,
    display_path: &str,
    suppression: &Suppression,
) -> LintDiagnostic {
    let (line, column) = line_col(source, suppression.offset);
    let rule = &suppression.rule;
    let scope = match suppression.coverage {
        Coverage::File => "in this file",
        Coverage::Span(_) => "in the statement below",
        Coverage::Line(_) => "on this line",
    };
    LintDiagnostic {
        severity: "error",
        rule: UNUSED_RULE,
        message: format!("`{rule}` is suppressed here but reports nothing {scope}."),
        line,
        column,
        file: Some(display_path.to_string()),
        suggested_fix: Some(format!("Remove `{rule}` from the suppression comment.")),
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::{lint_rq_file, LintDiagnostic};
    use crate::native::NativeFs;
    use crate::syntax::rq_file::RqFile;
    use std::path::PathBuf;

    fn lint_source(source: &str) -> Vec<LintDiagnostic> {
        let rq_file = RqFile::from_content_lenient(PathBuf::from("draft.rq"), source, &NativeFs);
        lint_rq_file(&rq_file, source, "draft.rq", &[], &[]).diagnostics
    }

    fn rules(diagnostics: &[LintDiagnostic]) -> Vec<&str> {
        diagnostics.iter().map(|d| d.rule).collect()
    }

    #[test]
    fn suppresses_the_named_rule_on_the_next_statement() {
        let target = lint_source(
            "ep widgets(\"http://localhost:8080/widgets\") {\n    \
             // rq-lint-ignore empty_url_string\n    \
             rq list(\"\");\n}\n",
        );
        assert!(target.is_empty(), "got {:?}", rules(&target));
    }

    #[test]
    fn keeps_the_same_rule_on_later_statements() {
        let target = lint_source(
            "ep widgets(\"http://localhost:8080/widgets\") {\n    \
             // rq-lint-ignore empty_url_string\n    \
             rq list(\"\");\n    \
             rq search(\"\");\n}\n",
        );
        assert_eq!(rules(&target), vec!["empty_url_string"]);
        assert_eq!(target[0].line, 4);
    }

    #[test]
    fn keeps_other_rules_on_the_suppressed_statement() {
        let target = lint_source(
            "ep widgets(\"http://localhost:8080/widgets\") {\n    \
             // rq-lint-ignore missing_body_on_write\n    \
             rq list(\"\");\n}\n",
        );
        assert!(
            rules(&target).contains(&"empty_url_string"),
            "got {:?}",
            rules(&target)
        );
    }

    #[test]
    fn accepts_a_comma_separated_list_followed_by_a_reason() {
        let target = lint_source(
            "ep widgets(\"http://localhost:8080/widgets\") {\n    \
             // rq-lint-ignore empty_url_string, missing_body_on_write: legacy endpoint\n    \
             rq post(\"\");\n}\n",
        );
        assert!(target.is_empty(), "got {:?}", rules(&target));
    }

    #[test]
    fn covers_a_statement_that_starts_with_attributes() {
        let target = lint_source(
            "ep widgets(\"http://localhost:8080/widgets\") {\n    \
             // rq-lint-ignore missing_body_on_write\n    \
             [method(POST)]\n    \
             rq create();\n}\n",
        );
        assert!(target.is_empty(), "got {:?}", rules(&target));
    }

    #[test]
    fn scopes_a_directive_inside_a_block_to_the_next_entry() {
        let target = lint_source(
            "env local {\n    \
             // rq-lint-ignore hardcoded_secret\n    \
             api_key: \"abc123\",\n    \
             client_secret: \"def456\",\n}\n",
        );
        assert_eq!(rules(&target), vec!["hardcoded_secret"]);
        assert_eq!(target[0].line, 4);
    }

    #[test]
    fn suppresses_the_named_rule_across_the_whole_file() {
        let target = lint_source(
            "// rq-lint-ignore-file multiple_endpoints_per_file\n\
             ep users(\"http://localhost:8080/users\") {\n    rq list();\n}\n\n\
             ep widgets(\"http://localhost:9090/widgets\") {\n    rq list();\n}\n",
        );
        assert!(target.is_empty(), "got {:?}", rules(&target));
    }

    #[test]
    fn reports_an_unknown_rule_at_its_position() {
        let target = lint_source("// rq-lint-ignore no_such_rule\nlet a = \"1\";\n");
        assert_eq!(rules(&target), vec!["invalid_lint_suppression"]);
        assert_eq!((target[0].line, target[0].column), (1, 19));
    }

    #[test]
    fn counts_columns_in_characters_after_multibyte_ids() {
        let target = lint_source("// rq-lint-ignore ñ, bogus\nlet a = \"1\";\n");
        let columns: Vec<usize> = target.iter().map(|d| d.column).collect();
        assert_eq!(columns, vec![19, 22]);
    }

    #[test]
    fn reports_a_directive_that_names_no_rule() {
        let target = lint_source("// rq-lint-ignore: no reason to lint\nlet a = \"1\";\n");
        assert_eq!(rules(&target), vec!["invalid_lint_suppression"]);
    }

    #[test]
    fn refuses_to_suppress_its_own_meta_rules() {
        let target = lint_source("// rq-lint-ignore unused_lint_suppression\nlet a = \"1\";\n");
        assert_eq!(rules(&target), vec!["invalid_lint_suppression"]);
    }

    #[test]
    fn suppresses_the_named_rule_on_the_line_of_a_trailing_directive() {
        let target = lint_source(
            "ep widgets(\"http://localhost:8080/widgets\") {\n    \
             rq list(\"\"); // rq-lint-ignore empty_url_string\n}\n",
        );
        assert!(target.is_empty(), "got {:?}", rules(&target));
    }

    #[test]
    fn limits_a_trailing_directive_to_its_own_line() {
        let target = lint_source(
            "env local {\n    \
             api_key: \"abc123\", // rq-lint-ignore hardcoded_secret: local mock\n    \
             client_secret: \"def456\",\n}\n",
        );
        assert_eq!(rules(&target), vec!["hardcoded_secret"]);
        assert_eq!(target[0].line, 3);
    }

    #[test]
    fn reports_a_trailing_directive_whose_finding_is_on_another_line_as_unused() {
        let target = lint_source(
            "ep widgets(\"http://localhost:8080/widgets\") {\n    \
             rq post(\n        \"/refresh\",\n    ); // rq-lint-ignore missing_body_on_write\n}\n",
        );
        let mut found = rules(&target);
        found.sort();
        assert_eq!(
            found,
            vec!["missing_body_on_write", "unused_lint_suppression"]
        );
    }

    #[test]
    fn reports_a_trailing_file_directive() {
        let target =
            lint_source("let a = \"1\"; // rq-lint-ignore-file multiple_endpoints_per_file\n");
        assert_eq!(rules(&target), vec!["invalid_lint_suppression"]);
    }

    #[test]
    fn reports_a_suppression_that_silences_nothing() {
        let target = lint_source("// rq-lint-ignore empty_url_string\nlet a = \"1\";\n");
        assert_eq!(rules(&target), vec!["unused_lint_suppression"]);
        assert_eq!((target[0].line, target[0].column), (1, 19));
    }

    #[test]
    fn reports_a_directive_with_no_statement_after_it_as_unused() {
        let target = lint_source("let a = \"1\";\n// rq-lint-ignore empty_url_string");
        assert_eq!(rules(&target), vec!["unused_lint_suppression"]);
    }

    #[test]
    fn ignores_directives_written_as_block_comments() {
        let target = lint_source(
            "ep widgets(\"http://localhost:8080/widgets\") {\n    \
             /* rq-lint-ignore empty_url_string */\n    \
             rq list(\"\");\n}\n",
        );
        assert_eq!(rules(&target), vec!["empty_url_string"]);
    }

    #[test]
    fn ignores_comments_that_only_start_like_a_directive() {
        let target = lint_source("// rq-lint-ignored for now\nlet a = \"1\";\n");
        assert!(target.is_empty(), "got {:?}", rules(&target));
    }
}
