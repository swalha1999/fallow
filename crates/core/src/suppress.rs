use oxc_ast::ast::Comment;

/// Issue kind for suppression matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueKind {
    UnusedFile,
    UnusedExport,
    UnusedType,
    UnusedDependency,
    UnusedDevDependency,
    UnusedEnumMember,
    UnusedClassMember,
    UnresolvedImport,
    UnlistedDependency,
    DuplicateExport,
}

impl IssueKind {
    /// Parse an issue kind from the string tokens used in CLI output and suppression comments.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "unused-file" => Some(Self::UnusedFile),
            "unused-export" => Some(Self::UnusedExport),
            "unused-type" => Some(Self::UnusedType),
            "unused-dependency" => Some(Self::UnusedDependency),
            "unused-dev-dependency" => Some(Self::UnusedDevDependency),
            "unused-enum-member" => Some(Self::UnusedEnumMember),
            "unused-class-member" => Some(Self::UnusedClassMember),
            "unresolved-import" => Some(Self::UnresolvedImport),
            "unlisted-dependency" => Some(Self::UnlistedDependency),
            "duplicate-export" => Some(Self::DuplicateExport),
            _ => None,
        }
    }

    /// Convert to a u8 discriminant for compact cache storage.
    pub fn to_discriminant(self) -> u8 {
        match self {
            Self::UnusedFile => 1,
            Self::UnusedExport => 2,
            Self::UnusedType => 3,
            Self::UnusedDependency => 4,
            Self::UnusedDevDependency => 5,
            Self::UnusedEnumMember => 6,
            Self::UnusedClassMember => 7,
            Self::UnresolvedImport => 8,
            Self::UnlistedDependency => 9,
            Self::DuplicateExport => 10,
        }
    }

    /// Reconstruct from a cache discriminant.
    pub fn from_discriminant(d: u8) -> Option<Self> {
        match d {
            1 => Some(Self::UnusedFile),
            2 => Some(Self::UnusedExport),
            3 => Some(Self::UnusedType),
            4 => Some(Self::UnusedDependency),
            5 => Some(Self::UnusedDevDependency),
            6 => Some(Self::UnusedEnumMember),
            7 => Some(Self::UnusedClassMember),
            8 => Some(Self::UnresolvedImport),
            9 => Some(Self::UnlistedDependency),
            10 => Some(Self::DuplicateExport),
            _ => None,
        }
    }
}

/// A suppression directive parsed from a source comment.
#[derive(Debug, Clone)]
pub struct Suppression {
    /// 1-based line this suppression applies to. 0 = file-wide suppression.
    pub line: u32,
    /// None = suppress all issue kinds on this line.
    pub kind: Option<IssueKind>,
}

/// Convert a byte offset to a 1-based line number.
fn byte_offset_to_line(source: &str, byte_offset: u32) -> u32 {
    let byte_offset = byte_offset as usize;
    let prefix = &source[..byte_offset.min(source.len())];
    prefix.bytes().filter(|&b| b == b'\n').count() as u32 + 1
}

/// Parse all fallow suppression comments from a file's comment list.
///
/// Supports:
/// - `// fallow-ignore-file` — suppress all issues in the file
/// - `// fallow-ignore-file unused-export` — suppress specific issue type for the file
/// - `// fallow-ignore-next-line` — suppress all issues on the next line
/// - `// fallow-ignore-next-line unused-export` — suppress specific issue type on the next line
pub fn parse_suppressions(comments: &[Comment], source: &str) -> Vec<Suppression> {
    let mut suppressions = Vec::new();

    for comment in comments {
        let content_span = comment.content_span();
        let text = &source
            [content_span.start as usize..content_span.end.min(source.len() as u32) as usize];
        let trimmed = text.trim();

        if let Some(rest) = trimmed.strip_prefix("fallow-ignore-file") {
            let rest = rest.trim();
            if rest.is_empty() {
                suppressions.push(Suppression {
                    line: 0,
                    kind: None,
                });
            } else if let Some(kind) = IssueKind::parse(rest) {
                suppressions.push(Suppression {
                    line: 0,
                    kind: Some(kind),
                });
            }
            // Unknown kind token: silently ignore (no suppression created)
        } else if let Some(rest) = trimmed.strip_prefix("fallow-ignore-next-line") {
            let rest = rest.trim();
            let comment_line = byte_offset_to_line(source, comment.span.start);
            let suppressed_line = comment_line + 1;

            if rest.is_empty() {
                suppressions.push(Suppression {
                    line: suppressed_line,
                    kind: None,
                });
            } else if let Some(kind) = IssueKind::parse(rest) {
                suppressions.push(Suppression {
                    line: suppressed_line,
                    kind: Some(kind),
                });
            }
            // Unknown kind token: silently ignore
        }
    }

    suppressions
}

/// Parse suppressions from raw source text using simple string scanning.
/// Used for SFC files where comment byte offsets don't correspond to the original file.
pub fn parse_suppressions_from_source(source: &str) -> Vec<Suppression> {
    let mut suppressions = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        // Match both // and /* */ style comments
        let comment_text = if let Some(rest) = trimmed.strip_prefix("//") {
            Some(rest.trim())
        } else if let Some(rest) = trimmed.strip_prefix("/*") {
            rest.strip_suffix("*/").map(|r| r.trim())
        } else {
            None
        };

        let Some(text) = comment_text else {
            continue;
        };

        if let Some(rest) = text.strip_prefix("fallow-ignore-file") {
            let rest = rest.trim();
            if rest.is_empty() {
                suppressions.push(Suppression {
                    line: 0,
                    kind: None,
                });
            } else if let Some(kind) = IssueKind::parse(rest) {
                suppressions.push(Suppression {
                    line: 0,
                    kind: Some(kind),
                });
            }
        } else if let Some(rest) = text.strip_prefix("fallow-ignore-next-line") {
            let rest = rest.trim();
            let suppressed_line = (line_idx as u32) + 2; // 1-based, next line

            if rest.is_empty() {
                suppressions.push(Suppression {
                    line: suppressed_line,
                    kind: None,
                });
            } else if let Some(kind) = IssueKind::parse(rest) {
                suppressions.push(Suppression {
                    line: suppressed_line,
                    kind: Some(kind),
                });
            }
        }
    }

    suppressions
}

/// Check if a specific issue at a given line should be suppressed.
pub fn is_suppressed(suppressions: &[Suppression], line: u32, kind: IssueKind) -> bool {
    suppressions.iter().any(|s| {
        // File-wide suppression
        if s.line == 0 {
            return s.kind.is_none() || s.kind == Some(kind);
        }
        // Line-specific suppression
        s.line == line && (s.kind.is_none() || s.kind == Some(kind))
    })
}

/// Check if the entire file is suppressed (for issue types that don't have line numbers).
pub fn is_file_suppressed(suppressions: &[Suppression], kind: IssueKind) -> bool {
    suppressions
        .iter()
        .any(|s| s.line == 0 && (s.kind.is_none() || s.kind == Some(kind)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_kind_from_str_all_variants() {
        assert_eq!(IssueKind::parse("unused-file"), Some(IssueKind::UnusedFile));
        assert_eq!(
            IssueKind::parse("unused-export"),
            Some(IssueKind::UnusedExport)
        );
        assert_eq!(IssueKind::parse("unused-type"), Some(IssueKind::UnusedType));
        assert_eq!(
            IssueKind::parse("unused-dependency"),
            Some(IssueKind::UnusedDependency)
        );
        assert_eq!(
            IssueKind::parse("unused-dev-dependency"),
            Some(IssueKind::UnusedDevDependency)
        );
        assert_eq!(
            IssueKind::parse("unused-enum-member"),
            Some(IssueKind::UnusedEnumMember)
        );
        assert_eq!(
            IssueKind::parse("unused-class-member"),
            Some(IssueKind::UnusedClassMember)
        );
        assert_eq!(
            IssueKind::parse("unresolved-import"),
            Some(IssueKind::UnresolvedImport)
        );
        assert_eq!(
            IssueKind::parse("unlisted-dependency"),
            Some(IssueKind::UnlistedDependency)
        );
        assert_eq!(
            IssueKind::parse("duplicate-export"),
            Some(IssueKind::DuplicateExport)
        );
    }

    #[test]
    fn issue_kind_from_str_unknown() {
        assert_eq!(IssueKind::parse("foo"), None);
        assert_eq!(IssueKind::parse(""), None);
    }

    #[test]
    fn discriminant_roundtrip() {
        for kind in [
            IssueKind::UnusedFile,
            IssueKind::UnusedExport,
            IssueKind::UnusedType,
            IssueKind::UnusedDependency,
            IssueKind::UnusedDevDependency,
            IssueKind::UnusedEnumMember,
            IssueKind::UnusedClassMember,
            IssueKind::UnresolvedImport,
            IssueKind::UnlistedDependency,
            IssueKind::DuplicateExport,
        ] {
            assert_eq!(
                IssueKind::from_discriminant(kind.to_discriminant()),
                Some(kind)
            );
        }
        assert_eq!(IssueKind::from_discriminant(0), None);
        assert_eq!(IssueKind::from_discriminant(11), None);
    }

    #[test]
    fn parse_file_wide_suppression() {
        let source = "// fallow-ignore-file\nexport const foo = 1;\n";
        let suppressions = parse_suppressions_from_source(source);
        assert_eq!(suppressions.len(), 1);
        assert_eq!(suppressions[0].line, 0);
        assert!(suppressions[0].kind.is_none());
    }

    #[test]
    fn parse_file_wide_suppression_with_kind() {
        let source = "// fallow-ignore-file unused-export\nexport const foo = 1;\n";
        let suppressions = parse_suppressions_from_source(source);
        assert_eq!(suppressions.len(), 1);
        assert_eq!(suppressions[0].line, 0);
        assert_eq!(suppressions[0].kind, Some(IssueKind::UnusedExport));
    }

    #[test]
    fn parse_next_line_suppression() {
        let source =
            "import { x } from './x';\n// fallow-ignore-next-line\nexport const foo = 1;\n";
        let suppressions = parse_suppressions_from_source(source);
        assert_eq!(suppressions.len(), 1);
        assert_eq!(suppressions[0].line, 3); // suppresses line 3 (the export)
        assert!(suppressions[0].kind.is_none());
    }

    #[test]
    fn parse_next_line_suppression_with_kind() {
        let source = "// fallow-ignore-next-line unused-export\nexport const foo = 1;\n";
        let suppressions = parse_suppressions_from_source(source);
        assert_eq!(suppressions.len(), 1);
        assert_eq!(suppressions[0].line, 2);
        assert_eq!(suppressions[0].kind, Some(IssueKind::UnusedExport));
    }

    #[test]
    fn parse_unknown_kind_ignored() {
        let source = "// fallow-ignore-next-line typo-kind\nexport const foo = 1;\n";
        let suppressions = parse_suppressions_from_source(source);
        assert!(suppressions.is_empty());
    }

    #[test]
    fn is_suppressed_file_wide() {
        let suppressions = vec![Suppression {
            line: 0,
            kind: None,
        }];
        assert!(is_suppressed(&suppressions, 5, IssueKind::UnusedExport));
        assert!(is_suppressed(&suppressions, 10, IssueKind::UnusedFile));
    }

    #[test]
    fn is_suppressed_file_wide_specific_kind() {
        let suppressions = vec![Suppression {
            line: 0,
            kind: Some(IssueKind::UnusedExport),
        }];
        assert!(is_suppressed(&suppressions, 5, IssueKind::UnusedExport));
        assert!(!is_suppressed(&suppressions, 5, IssueKind::UnusedType));
    }

    #[test]
    fn is_suppressed_line_specific() {
        let suppressions = vec![Suppression {
            line: 5,
            kind: None,
        }];
        assert!(is_suppressed(&suppressions, 5, IssueKind::UnusedExport));
        assert!(!is_suppressed(&suppressions, 6, IssueKind::UnusedExport));
    }

    #[test]
    fn is_suppressed_line_and_kind() {
        let suppressions = vec![Suppression {
            line: 5,
            kind: Some(IssueKind::UnusedExport),
        }];
        assert!(is_suppressed(&suppressions, 5, IssueKind::UnusedExport));
        assert!(!is_suppressed(&suppressions, 5, IssueKind::UnusedType));
        assert!(!is_suppressed(&suppressions, 6, IssueKind::UnusedExport));
    }

    #[test]
    fn is_suppressed_empty() {
        assert!(!is_suppressed(&[], 5, IssueKind::UnusedExport));
    }

    #[test]
    fn is_file_suppressed_works() {
        let suppressions = vec![Suppression {
            line: 0,
            kind: None,
        }];
        assert!(is_file_suppressed(&suppressions, IssueKind::UnusedFile));

        let suppressions = vec![Suppression {
            line: 0,
            kind: Some(IssueKind::UnusedFile),
        }];
        assert!(is_file_suppressed(&suppressions, IssueKind::UnusedFile));
        assert!(!is_file_suppressed(&suppressions, IssueKind::UnusedExport));

        // Line-specific suppression should not count as file-wide
        let suppressions = vec![Suppression {
            line: 5,
            kind: None,
        }];
        assert!(!is_file_suppressed(&suppressions, IssueKind::UnusedFile));
    }

    #[test]
    fn parse_oxc_comments() {
        use oxc_allocator::Allocator;
        use oxc_parser::Parser;
        use oxc_span::SourceType;

        let source = "// fallow-ignore-file\n// fallow-ignore-next-line unused-export\nexport const foo = 1;\nexport const bar = 2;\n";
        let allocator = Allocator::default();
        let parser_return = Parser::new(&allocator, source, SourceType::mjs()).parse();

        let suppressions = parse_suppressions(&parser_return.program.comments, source);
        assert_eq!(suppressions.len(), 2);

        // File-wide suppression
        assert_eq!(suppressions[0].line, 0);
        assert!(suppressions[0].kind.is_none());

        // Next-line suppression with kind
        assert_eq!(suppressions[1].line, 3); // suppresses line 3 (export const foo)
        assert_eq!(suppressions[1].kind, Some(IssueKind::UnusedExport));
    }

    #[test]
    fn parse_block_comment_suppression() {
        let source = "/* fallow-ignore-file */\nexport const foo = 1;\n";
        let suppressions = parse_suppressions_from_source(source);
        assert_eq!(suppressions.len(), 1);
        assert_eq!(suppressions[0].line, 0);
        assert!(suppressions[0].kind.is_none());
    }
}
