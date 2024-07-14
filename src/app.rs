use crate::config::Config;

use std::{
    collections::HashMap,
    fmt::Write,
    path::{Path, PathBuf},
};

use console::{style, Style, Term};
use tree_sitter::{Node, Parser, Query, QueryCursor, Range, Tree};

pub struct App {
    config: Config,
    language: tree_sitter::Language,
    path: PathBuf,
    query: Option<Query>,
    query_path: Option<PathBuf>,
    src: Vec<u8>,
    tree: Tree,
}

impl App {
    pub fn new<'a, P: AsRef<Path>>(
        src: &'a [u8],
        path: P,
        query_path: Option<P>,
        language: tree_sitter::Language,
    ) -> Self {
        let path = path.as_ref().to_owned();

        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();

        let tree = parser.parse(&src, None).unwrap();
        let query_path = query_path.map(|q| q.as_ref().to_owned());
        let query = query_path.as_ref().map(|p| {
            let query_src = std::fs::read_to_string(&p).expect("unable to read query");
            Query::new(&language, &query_src).expect("query parse error")
        });

        Self {
            config: Default::default(),
            path,
            query,
            query_path,
            src: src.to_owned(),
            tree,
            language,
        }
    }

    pub fn draw(&self) {
        let term = Term::stdout();
        term.clear_screen().unwrap();
        let mut done = false;
        let mut depth = 0;
        let mut in_capture: Option<Range> = None;
        let mut cursor = self.tree.walk();

        let capture_names = self
            .query
            .as_ref()
            .map(|q| q.capture_names())
            .unwrap_or_default();
        let capture_map = self
            .query
            .as_ref()
            .map(|query| {
                QueryCursor::new()
                    .matches(&query, self.tree.root_node(), self.src.as_slice())
                    .flat_map(|match_| match_.captures)
                    .fold(
                        HashMap::new(),
                        |mut map: HashMap<Node, Vec<u32>>, capture| {
                            map.entry(capture.node)
                                .and_modify(|idxs| idxs.push(capture.index))
                                .or_insert_with(|| vec![capture.index]);
                            map
                        },
                    )
            })
            .unwrap_or_default();

        while !done {
            let node = cursor.node();
            let mut tree_string = String::new();
            in_capture = match in_capture {
                Some(range)
                    if !contains(&range, &node.range()) && capture_map.contains_key(&node) =>
                {
                    Some(node.range())
                }
                Some(range) if !contains(&range, &node.range()) => None,
                None if capture_map.contains_key(&node) => Some(node.range()),
                i => i,
            };

            write!(
                tree_string,
                "{}",
                (if in_capture.is_some() {
                    Style::new().on_yellow().on_bright()
                } else {
                    Style::new()
                })
                .bright()
                .black()
                .apply_to(
                    format!("{}{}", "|", " ".repeat(self.config.indent_level))
                        .repeat(depth as usize)
                )
            )
            .unwrap();

            if self.config.show_field_name {
                if let Some(f) = cursor.field_name() {
                    write!(
                        tree_string,
                        "{} ",
                        if in_capture.is_some() {
                            Style::new().on_yellow().on_bright()
                        } else {
                            Style::new()
                        }
                        .yellow()
                        .apply_to(f)
                    )
                    .unwrap()
                }
            }

            write!(
                tree_string,
                "{} ",
                if node.is_error() {
                    Style::new().red()
                } else if in_capture.is_some() {
                    Style::new().on_yellow().on_bright()
                } else {
                    Style::new()
                }
                .apply_to(node.kind()),
            )
            .unwrap();

            if let Some(idxs) = capture_map.get(&node) {
                for index in idxs {
                    write!(
                        tree_string,
                        "@{} ",
                        style(capture_names[*index as usize]).magenta()
                    )
                    .unwrap();
                }
            }

            if self.config.show_ranges {
                let range = node.range();
                write!(
                    tree_string,
                    " {}",
                    style(format!("{:?}..{:?}", range.start_byte, range.end_byte,))
                        .bright()
                        .black()
                )
                .unwrap();
            }

            if self.config.show_src {
                write!(
                    tree_string,
                    " {:.?}",
                    style(node.utf8_text(&self.src).unwrap()).cyan()
                )
                .unwrap();
            }

            term.write_line(&tree_string).unwrap();
            term.clear_to_end_of_screen().unwrap();

            if cursor.goto_first_child() {
                depth += 1;
                continue;
            }
            if cursor.goto_next_sibling() {
                continue;
            }

            loop {
                if !cursor.goto_parent() {
                    done = true;
                    break;
                } else {
                    depth -= 1;
                }

                if cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        // see https://github.com/console-rs/console/issues/36#issuecomment-624731432
        // for the reasoning behing this hackjob

        term.write_line("\n(>) increase indent").unwrap();
        term.clear_to_end_of_screen().unwrap();

        term.write_line("(<) decrease indent ").unwrap();
        term.clear_to_end_of_screen().unwrap();

        term.write_line("(n) toggle ranges").unwrap();
        term.clear_to_end_of_screen().unwrap();

        term.write_line("(s) toggle source text").unwrap();
        term.clear_to_end_of_screen().unwrap();

        term.write_line("(r) reload from disk").unwrap();
        term.clear_to_end_of_screen().unwrap();

        term.write_line("(C-c) quit").unwrap();
        term.clear_to_end_of_screen().unwrap();
    }

    pub fn increase_indent(&mut self) {
        self.config.indent_level = self.config.indent_level.saturating_add(1);
    }

    pub fn decrease_indent(&mut self) {
        self.config.indent_level = self.config.indent_level.saturating_sub(1);
    }

    pub fn toggle_ranges(&mut self) {
        self.config.show_ranges = !self.config.show_ranges;
    }

    pub fn toggle_source(&mut self) {
        self.config.show_src = !self.config.show_src;
    }

    pub fn reload(&mut self) {
        let src = std::fs::read_to_string(&self.path).unwrap();
        let new = Self::new(
            src.as_bytes(),
            &self.path,
            self.query_path.as_ref(),
            self.language.clone(),
        );
        *self = Self {
            config: self.config,
            ..new
        };
    }
}

// does a encompass b
fn contains(a: &Range, b: &Range) -> bool {
    a.start_byte <= b.start_byte && a.end_byte >= b.end_byte
}
