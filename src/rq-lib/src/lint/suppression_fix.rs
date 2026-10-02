use crate::lint::line_col;
use crate::lint::suppression::{
    parse_comment, starts_line, statement_range, ParsedComment, IGNORE, IGNORE_FILE,
};
use crate::syntax::token::{Token, TokenType};
use serde::{Deserialize, Serialize};
use std::ops::Range;

const FILE_SCOPE_EXCLUDED: &[&str] = &["hardcoded_secret"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuppressionScope {
    Line,
    Statement,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceEdit {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub new_text: String,
}

type Insertion = (Range<usize>, String);

struct Lines<'a> {
    source: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    fn new(source: &'a str) -> Self {
        let starts = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(index, _)| index + 1))
            .collect();
        Lines { source, starts }
    }

    fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start <= offset)
    }

    fn start(&self, line: usize) -> Option<usize> {
        self.starts.get(line.checked_sub(1)?).copied()
    }

    fn offset(&self, line: usize, column: usize) -> Option<usize> {
        let start = self.start(line)?;
        self.source
            .get(start..)?
            .char_indices()
            .map(|(index, _)| start + index)
            .chain(std::iter::once(self.source.len()))
            .nth(column.checked_sub(1)?)
    }

    fn eol(&self) -> &'static str {
        if self.source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        }
    }

    fn edit(&self, (range, new_text): Insertion) -> SourceEdit {
        let (start_line, start_column) = line_col(self.source, range.start);
        let (end_line, end_column) = line_col(self.source, range.end);
        SourceEdit {
            start_line,
            start_column,
            end_line,
            end_column,
            new_text,
        }
    }
}

pub fn suppression_edit(
    source: &str,
    known_rules: &[&str],
    rule: &str,
    line: usize,
    column: usize,
    scope: SuppressionScope,
) -> Option<SourceEdit> {
    if !known_rules.contains(&rule) {
        return None;
    }
    let tokens = crate::syntax::tokenize(source).ok()?;
    let lines = Lines::new(source);
    let insertion = match scope {
        SuppressionScope::Line => line_insertion(&lines, &tokens, rule, line),
        SuppressionScope::Statement => statement_insertion(&lines, &tokens, rule, line, column),
        SuppressionScope::File => file_insertion(&lines, &tokens, rule),
    }?;
    Some(lines.edit(insertion))
}

pub fn unused_suppression_removal(source: &str, line: usize, column: usize) -> Option<SourceEdit> {
    let tokens = crate::syntax::tokenize(source).ok()?;
    let lines = Lines::new(source);
    let offset = lines.offset(line, column)?;
    let (index, token) = tokens
        .iter()
        .enumerate()
        .find(|(_, t)| t.token_type == TokenType::Comment && t.span.contains(&offset))?;
    let parsed = parse_comment(&token.value)?;
    let position = parsed
        .rules
        .iter()
        .position(|(start, _)| token.span.start + start == offset)?;
    let range = if parsed.rules.len() > 1 {
        rule_removal(token, &parsed, position)?
    } else {
        directive_removal(&lines, &tokens, index, token)?
    };
    Some(lines.edit((range, String::new())))
}

fn line_insertion(lines: &Lines, tokens: &[Token], rule: &str, line: usize) -> Option<Insertion> {
    let on_line: Vec<(usize, &Token)> = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| !matches!(t.token_type, TokenType::Whitespace | TokenType::Newline))
        .filter(|(_, t)| lines.line_of(t.span.start) == line)
        .collect();
    if let Some((index, comment)) = on_line
        .iter()
        .find(|(_, t)| t.token_type == TokenType::Comment && t.value.starts_with("//"))
    {
        let parsed = parse_comment(&comment.value)?;
        if parsed.keyword != IGNORE || starts_line(tokens, *index) {
            return None;
        }
        return merge(comment, &parsed, rule);
    }
    if !on_line
        .iter()
        .any(|(_, t)| t.token_type != TokenType::Comment)
    {
        return None;
    }
    let (_, last) = on_line.last()?;
    if lines.line_of(last.span.end) != line {
        return None;
    }
    let at = last.span.end;
    Some((at..at, format!(" // {IGNORE} {rule}")))
}

fn statement_insertion(
    lines: &Lines,
    tokens: &[Token],
    rule: &str,
    line: usize,
    column: usize,
) -> Option<Insertion> {
    let line_start = lines.start(line)?;
    let (index, first) = tokens
        .iter()
        .enumerate()
        .find(|(_, t)| is_significant(t) && t.span.start >= line_start)?;
    if lines.line_of(first.span.start) != line {
        return None;
    }
    let indent = lines.source.get(line_start..first.span.start)?;
    if !indent.chars().all(char::is_whitespace) {
        return None;
    }
    if matches!(first.value.as_str(), ")" | "]" | "}") {
        return None;
    }
    let range = statement_range(tokens, index)?;
    let position = (line, column);
    let covered = line_col(lines.source, range.start) <= position
        && position < line_col(lines.source, range.end);
    if !covered {
        return None;
    }
    if let Some((directive, parsed)) = directive_above(tokens, index) {
        return merge(directive, &parsed, rule);
    }
    let eol = lines.eol();
    Some((
        line_start..line_start,
        format!("{indent}// {IGNORE} {rule}{eol}"),
    ))
}

fn file_insertion(lines: &Lines, tokens: &[Token], rule: &str) -> Option<Insertion> {
    if FILE_SCOPE_EXCLUDED.contains(&rule) {
        return None;
    }
    let existing: Vec<(&Token, ParsedComment)> = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.token_type == TokenType::Comment)
        .filter_map(|(index, t)| {
            let parsed = parse_comment(&t.value)?;
            (parsed.keyword == IGNORE_FILE && starts_line(tokens, index)).then_some((t, parsed))
        })
        .collect();
    if existing.iter().any(|(_, parsed)| names(parsed, rule)) {
        return None;
    }
    if let Some((directive, parsed)) = existing.first() {
        return merge(directive, parsed, rule);
    }
    let eol = lines.eol();
    Some((0..0, format!("// {IGNORE_FILE} {rule}{eol}")))
}

fn directive_above(tokens: &[Token], index: usize) -> Option<(&Token, ParsedComment)> {
    tokens
        .iter()
        .enumerate()
        .take(index)
        .rev()
        .take_while(|(_, t)| !is_significant(t))
        .filter(|(_, t)| t.token_type == TokenType::Comment)
        .find_map(|(position, t)| {
            let parsed = parse_comment(&t.value)?;
            (parsed.keyword == IGNORE && starts_line(tokens, position)).then_some((t, parsed))
        })
}

fn merge(directive: &Token, parsed: &ParsedComment, rule: &str) -> Option<Insertion> {
    if names(parsed, rule) {
        return None;
    }
    let (at, text) = match parsed.rules.last() {
        Some((start, last)) => (start + last.len(), format!(", {rule}")),
        None => (parsed.keyword_end, format!(" {rule}")),
    };
    let at = directive.span.start + at;
    Some((at..at, text))
}

fn rule_removal(
    directive: &Token,
    parsed: &ParsedComment,
    position: usize,
) -> Option<Range<usize>> {
    let (start, rule) = parsed.rules.get(position)?;
    let range = match position.checked_sub(1) {
        Some(previous) => {
            let (previous_start, previous_rule) = parsed.rules.get(previous)?;
            previous_start + previous_rule.len()..start + rule.len()
        }
        None => *start..parsed.rules.get(1)?.0,
    };
    let base = directive.span.start;
    Some(base + range.start..base + range.end)
}

fn directive_removal(
    lines: &Lines,
    tokens: &[Token],
    index: usize,
    directive: &Token,
) -> Option<Range<usize>> {
    if starts_line(tokens, index) {
        let line = lines.line_of(directive.span.start);
        let start = lines.start(line)?;
        let end = lines.start(line + 1).unwrap_or(lines.source.len());
        return Some(start..end);
    }
    let code_end = tokens
        .iter()
        .take(index)
        .rev()
        .find(|t| t.token_type != TokenType::Whitespace)?
        .span
        .end;
    let line_break = if directive.value.ends_with('\r') {
        1
    } else {
        0
    };
    Some(code_end..directive.span.end - line_break)
}

fn names(parsed: &ParsedComment, rule: &str) -> bool {
    parsed.rules.iter().any(|(_, named)| named == rule)
}

fn is_significant(token: &Token) -> bool {
    !matches!(
        token.token_type,
        TokenType::Whitespace | TokenType::Newline | TokenType::Comment
    )
}

#[cfg(test)]
mod tests {
    use super::{Lines, SourceEdit, SuppressionScope};
    use crate::lint::{lint_rq_file, suppression_edit, unused_suppression_removal, LintDiagnostic};
    use crate::native::NativeFs;
    use crate::syntax::rq_file::RqFile;
    use std::path::PathBuf;

    const EMPTY_URL: &str =
        "ep widgets(\"http://localhost:8080/widgets\") {\n    rq list(\"\");\n}\n";

    fn lint_source(source: &str) -> Vec<LintDiagnostic> {
        let rq_file = RqFile::from_content_lenient(PathBuf::from("draft.rq"), source, &NativeFs);
        lint_rq_file(&rq_file, source, "draft.rq", &[], &[]).diagnostics
    }

    fn apply(source: &str, edit: &SourceEdit) -> String {
        let lines = Lines::new(source);
        let start = lines
            .offset(edit.start_line, edit.start_column)
            .expect("start");
        let end = lines.offset(edit.end_line, edit.end_column).expect("end");
        format!("{}{}{}", &source[..start], edit.new_text, &source[end..])
    }

    fn suppress(source: &str, rule: &str, scope: SuppressionScope) -> Option<String> {
        let finding = lint_source(source)
            .into_iter()
            .find(|d| d.rule == rule)
            .expect("finding to suppress");
        let edit = suppression_edit(source, rule, finding.line, finding.column, scope)?;
        Some(apply(source, &edit))
    }

    fn remove_unused(source: &str) -> String {
        let finding = lint_source(source)
            .into_iter()
            .find(|d| d.rule == "unused_lint_suppression")
            .expect("unused suppression");
        let edit =
            unused_suppression_removal(source, finding.line, finding.column).expect("removal edit");
        apply(source, &edit)
    }

    #[test]
    fn reads_scopes_by_their_snake_case_names() {
        let target: Vec<SuppressionScope> =
            serde_json::from_str(r#"["line", "statement", "file"]"#).expect("scopes");
        assert_eq!(
            target,
            vec![
                SuppressionScope::Line,
                SuppressionScope::Statement,
                SuppressionScope::File
            ]
        );
    }

    #[test]
    fn appends_a_trailing_directive_to_the_line() {
        let target = suppress(EMPTY_URL, "empty_url_string", SuppressionScope::Line);
        assert_eq!(
            target.as_deref(),
            Some(
                "ep widgets(\"http://localhost:8080/widgets\") {\n    \
                 rq list(\"\"); // rq-lint-ignore empty_url_string\n}\n"
            )
        );
    }

    #[test]
    fn clears_the_finding_with_a_line_directive() {
        let target = suppress(EMPTY_URL, "empty_url_string", SuppressionScope::Line).expect("edit");
        assert!(lint_source(&target).is_empty());
    }

    #[test]
    fn extends_an_existing_trailing_directive() {
        let source = "ep widgets(\"http://localhost:8080/widgets\") {\n    \
                      rq post(\"\"); // rq-lint-ignore missing_body_on_write: legacy\n}\n";
        let target = suppress(source, "empty_url_string", SuppressionScope::Line).expect("edit");
        assert!(
            target.contains("// rq-lint-ignore missing_body_on_write, empty_url_string: legacy"),
            "got {target}"
        );
    }

    #[test]
    fn offers_no_line_directive_after_a_plain_comment() {
        let source = "ep widgets(\"http://localhost:8080/widgets\") {\n    \
                      rq list(\"\"); // todo\n}\n";
        assert_eq!(
            suppress(source, "empty_url_string", SuppressionScope::Line),
            None
        );
    }

    #[test]
    fn offers_no_line_directive_on_a_line_that_ends_inside_a_string() {
        let source = "let body = \"first\nsecond\";\n";
        let target = suppression_edit(source, "empty_url_string", 1, 1, SuppressionScope::Line);
        assert_eq!(target, None);
    }

    #[test]
    fn inserts_a_directive_above_the_statement_with_its_indentation() {
        let target = suppress(EMPTY_URL, "empty_url_string", SuppressionScope::Statement);
        assert_eq!(
            target.as_deref(),
            Some(
                "ep widgets(\"http://localhost:8080/widgets\") {\n    \
                 // rq-lint-ignore empty_url_string\n    \
                 rq list(\"\");\n}\n"
            )
        );
    }

    #[test]
    fn clears_the_finding_with_a_statement_directive() {
        let target =
            suppress(EMPTY_URL, "empty_url_string", SuppressionScope::Statement).expect("edit");
        assert!(lint_source(&target).is_empty());
    }

    #[test]
    fn extends_the_directive_already_above_the_statement_keeping_its_reason() {
        let source = "ep widgets(\"http://localhost:8080/widgets\") {\n    \
                      // rq-lint-ignore missing_body_on_write: legacy\n    \
                      rq post(\"\");\n}\n";
        let target =
            suppress(source, "empty_url_string", SuppressionScope::Statement).expect("edit");
        assert!(
            target.contains("    // rq-lint-ignore missing_body_on_write, empty_url_string: legacy\n    rq post"),
            "got {target}"
        );
    }

    #[test]
    fn offers_no_statement_directive_on_a_closing_line() {
        let target = suppression_edit(
            EMPTY_URL,
            "empty_url_string",
            3,
            1,
            SuppressionScope::Statement,
        );
        assert_eq!(target, None);
    }

    #[test]
    fn inserts_a_file_directive_at_the_top() {
        let target = suppress(EMPTY_URL, "empty_url_string", SuppressionScope::File).expect("edit");
        assert!(
            target.starts_with("// rq-lint-ignore-file empty_url_string\nep widgets"),
            "got {target}"
        );
    }

    #[test]
    fn extends_an_existing_file_directive() {
        let source = format!("// rq-lint-ignore-file missing_body_on_write\n{EMPTY_URL}");
        let target = suppression_edit(&source, "empty_url_string", 3, 13, SuppressionScope::File)
            .map(|edit| apply(&source, &edit))
            .expect("edit");
        assert!(
            target.starts_with(
                "// rq-lint-ignore-file missing_body_on_write, empty_url_string\nep widgets"
            ),
            "got {target}"
        );
    }

    #[test]
    fn offers_no_file_directive_for_hardcoded_secret() {
        let source = "env local {\n    api_key: \"abc123\",\n}\n";
        assert_eq!(
            suppress(source, "hardcoded_secret", SuppressionScope::File),
            None
        );
    }

    #[test]
    fn offers_no_directive_for_a_meta_rule() {
        let source = "// rq-lint-ignore bogus\nlet a = \"1\";\n";
        assert_eq!(
            suppress(source, "invalid_lint_suppression", SuppressionScope::Line),
            None
        );
    }

    #[test]
    fn keeps_crlf_line_endings() {
        let source = EMPTY_URL.replace('\n', "\r\n");
        let target =
            suppress(&source, "empty_url_string", SuppressionScope::Statement).expect("edit");
        assert!(
            target.contains("    // rq-lint-ignore empty_url_string\r\n    rq list"),
            "got {target:?}"
        );
    }

    #[test]
    fn removes_an_unused_directive_on_its_own_line_with_the_line() {
        let target =
            remove_unused("let a = \"1\";\n// rq-lint-ignore empty_url_string\nlet b = \"2\";\n");
        assert_eq!(target, "let a = \"1\";\nlet b = \"2\";\n");
    }

    #[test]
    fn removes_an_unused_trailing_directive_with_its_leading_space() {
        let target = remove_unused("let a = \"1\"; // rq-lint-ignore empty_url_string\n");
        assert_eq!(target, "let a = \"1\";\n");
    }

    #[test]
    fn keeps_crlf_line_endings_when_removing_a_trailing_directive() {
        let target = remove_unused(
            "let a = \"1\"; // rq-lint-ignore empty_url_string\r\nlet b = \"2\";\r\n",
        );
        assert_eq!(target, "let a = \"1\";\r\nlet b = \"2\";\r\n");
    }

    #[test]
    fn removes_only_the_unused_rule_when_it_comes_first() {
        let source = "ep widgets(\"http://localhost:8080/widgets\") {\n    \
                      // rq-lint-ignore missing_body_on_write, empty_url_string: legacy\n    \
                      rq list(\"\");\n}\n";
        let target = remove_unused(source);
        assert!(
            target.contains("// rq-lint-ignore empty_url_string: legacy\n"),
            "got {target}"
        );
    }

    #[test]
    fn removes_only_the_unused_rule_when_it_comes_later() {
        let source = "ep widgets(\"http://localhost:8080/widgets\") {\n    \
                      // rq-lint-ignore empty_url_string, missing_body_on_write\n    \
                      rq list(\"\");\n}\n";
        let target = remove_unused(source);
        assert!(lint_source(&target).is_empty(), "got {target}");
    }
}
