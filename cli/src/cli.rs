use std::collections::HashMap;
use std::io::stderr;
use std::path::PathBuf;

use clap::Parser;
use dbg_pls::color;

use analyzer::diagnostic::Diagnostic;
use analyzer::name::Name;
use analyzer::reef::Externals;
use analyzer::relations::SourceId;
use analyzer::Analyzer;
use compiler::{compile, CompilerOptions, SourceLineProvider};
use context::source::ContentId;
use vm::{VmError, VM};

use crate::disassemble::display_bytecode;
use crate::pipeline::{FileImportError, PipelineStatus, SourceHolder, SourcesCache};
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

pub struct CachedSourceLocationLineProvider {
    lines: HashMap<ContentId, Vec<usize>>,
}

impl CachedSourceLocationLineProvider {
    fn compute(contents: &[ContentId], sources: &impl SourceHolder) -> Self {
        let lines = contents
            .iter()
            .map(|&content_id| {
                let source = sources.get_source(content_id).expect("unknown content id");

                let source_start_addr = source.source.as_ptr() as usize;

                let source_lines_starts: Vec<_> = source
                    .source
                    .lines()
                    .map(|line| line.as_ptr() as usize - source_start_addr)
                    .collect();

                (content_id, source_lines_starts)
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

#[must_use = "The pipeline status should be checked"]
#[allow(clippy::too_many_arguments)]
pub fn use_pipeline(
    entry_point: &Name,
    starting_page: SourceId,
    analyzer: &Analyzer<'_>,
    externals: &Externals,
    vm: &mut VM,
    diagnostics: Vec<Diagnostic>,
    errors: Vec<FileImportError>,
    sources: &SourcesCache,
    config: &Cli,
) -> PipelineStatus {
    if errors.is_empty() && analyzer.resolution.engine.is_empty() {
        eprintln!("No module found for entry point {entry_point}");
        return PipelineStatus::IoError;
    }

    let reef = externals.current;

    let mut import_status = PipelineStatus::Success;
    for error in errors {
        match error {
            FileImportError::IO { inner, path } => {
                eprintln!("Couldn't read {}: {inner}", path.display());
                import_status = PipelineStatus::IoError;
            }
            FileImportError::Parse(report) => {
                for error in report.errors {
                    let source = sources
                        .get(reef)
                        .and_then(|importer| importer.get_source(report.source))
                        .unwrap();
                    display_parse_error(source, error, &mut stderr())
                        .expect("IO error when reporting diagnostics");
                }

                // Prefer the IO error over a generic failure
                if import_status != PipelineStatus::IoError {
                    import_status = PipelineStatus::AnalysisError;
                }
            }
        }
    }
    if import_status != PipelineStatus::Success {
        return import_status;
    }

    let engine = &analyzer.resolution.engine;
    if config.ast {
        for ast in engine
            .environments()
            .filter(|(_, env)| env.parent.is_none())
            .filter_map(|(id, _)| engine.get_expression(id))
        {
            println!("{}", color(ast))
        }
    }

    let mut stderr = stderr();
    let had_errors = !diagnostics.is_empty();
    for diagnostic in diagnostics {
        display_diagnostic(
            externals,
            engine,
            externals.current,
            sources,
            diagnostic,
            &mut stderr,
        )
        .expect("IO errors when reporting diagnostic");
    }

    if had_errors {
        return PipelineStatus::AnalysisError;
    }
    let mut bytes = Vec::new();

    let importer = sources.get(reef).expect("unknown reef");
    let contents = importer.list_content_ids();
    let lines = CachedSourceLocationLineProvider::compute(&contents, importer);

    compile(
        &analyzer.engine,
        &analyzer.typing,
        &analyzer.resolution.relations,
        &analyzer.resolution.engine,
        externals,
        externals.current,
        starting_page,
        &mut bytes,
        CompilerOptions {
            line_provider: Some(&lines),
            last_page_storage_var: None,
        },
    )
    .expect("write failed");

    if config.disassemble {
        display_bytecode(&bytes);
    }

    let mut run_status = PipelineStatus::Success;
    if !config.no_execute {
        vm.register(&bytes)
            .expect("compilation created invalid bytecode");
        drop(bytes);
        match unsafe { vm.run() } {
            Ok(()) => {}
            Err(VmError::Panic) => run_status = PipelineStatus::ExecutionFailure,
            Err(VmError::Internal) => panic!("VM internal error"),
        }
    }
    run_status
}
