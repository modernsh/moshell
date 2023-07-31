use clap::Parser;
use dbg_pls::color;
use std::collections::HashMap;
use std::io::stderr;
use std::path::PathBuf;

use analyzer::analyze;
use analyzer::importer::ASTImporter;
use analyzer::name::Name;
use compiler::{compile, SourceLineProvider};
use context::source::ContentId;
use vm::{execute_bytecode, VmError};

use crate::disassemble::display_bytecode;
use crate::pipeline::{ErrorReporter, FileImportError, SourceHolder};
use crate::report::{display_diagnostic, display_parse_error};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Defines the source file to parse
    #[arg(short, long, value_name = "FILE")]
    pub(crate) source: Option<PathBuf>,

    /// Prints the generated bytecode
    #[arg(short = 'D', long)]
    pub(crate) disassemble: bool,

    /// Display a textual representation of the abstract syntax tree
    #[arg(short = 'A', long)]
    pub(crate) ast: bool,

    /// Do not execute the code
    #[arg(long = "no-execute")]
    pub(crate) no_execute: bool,
}

struct CachedSourceLocationLineProvider {
    lines: HashMap<ContentId, Vec<usize>>,
}

impl CachedSourceLocationLineProvider {
    fn compute(contents: &[ContentId], sources: &impl SourceHolder) -> Self {
        let lines = contents
            .iter()
            .map(|content_id| {
                let source = sources.get_source(*content_id).expect("unknown content id");

                let source_start_addr = source.source.as_ptr() as usize;

                let source_lines_starts: Vec<_> = source
                    .source
                    .lines()
                    .map(|line| line.as_ptr() as usize - source_start_addr)
                    .collect();

                (*content_id, source_lines_starts)
            })
            .collect();

        Self { lines }
    }
}

impl SourceLineProvider for CachedSourceLocationLineProvider {
    fn get_line(&self, content: ContentId, pos: usize) -> Option<usize> {
        self.lines.get(&content).map(|lines| {
            lines
                .binary_search(&pos)
                .map(|line| line + 1)
                .unwrap_or_else(|line| line)
        })
    }
}

pub fn resolve_and_execute<'a>(
    entry_point: Name,
    importer: &mut (impl ASTImporter<'a> + ErrorReporter),
    config: &Cli,
) -> bool {
    let mut analyzer = analyze(entry_point.clone(), importer);
    let result = &analyzer.resolution;

    let errors = importer.take_errors();
    if errors.is_empty() && result.engine.is_empty() {
        eprintln!("No module found for entry point {entry_point}");
        return true;
    }

    let has_errors = !errors.is_empty();
    for error in errors {
        match error {
            FileImportError::IO(err) => {
                eprintln!("IO error: {err}");
            }
            FileImportError::Parse(report) => {
                for error in report.errors {
                    display_parse_error(
                        importer.get_source(report.source).unwrap(),
                        error,
                        &mut stderr(),
                    )
                    .expect("IO error when reporting diagnostics");
                }
            }
        }
    }
    if has_errors {
        return true;
    }

    if config.ast {
        for ast in result
            .engine
            .environments()
            .filter(|(_, env)| env.parent.is_none())
            .filter_map(|(id, _)| result.engine.get_expression(id))
        {
            println!("{}", color(ast))
        }
    }

    let diagnostics = analyzer.take_diagnostics();
    let result = &analyzer.resolution;
    if diagnostics.is_empty() {
        let mut bytes = Vec::new();
        let contents = importer.list_content_ids();
        let lines = CachedSourceLocationLineProvider::compute(&contents, importer);
        compile(
            &analyzer.engine,
            &result.engine,
            &result.relations,
            &mut bytes,
            Some(&lines),
        )
        .expect("write failed");

        if config.disassemble {
            display_bytecode(&bytes);
        }

        if !config.no_execute {
            execute(&bytes);
        }

        return false;
    }

    let mut stderr = stderr();
    let had_errors = !diagnostics.is_empty();
    for diagnostic in diagnostics {
        display_diagnostic(&result.engine, importer, diagnostic, &mut stderr)
            .expect("IO errors when reporting diagnostic");
    }
    had_errors
}

fn execute(bytes: &[u8]) {
    if unsafe { execute_bytecode(bytes) } == Err(VmError::Internal) {
        panic!("VM internal error");
    }
}
