use std::collections::HashMap;
use std::fs::read_to_string;
use std::io;
use std::path::{PathBuf, MAIN_SEPARATOR_STR};
use std::process::{ExitCode, Termination};

use analyzer::importer::{ASTImporter, ImportResult, Imported};
use analyzer::name::Name;

use ast::group::Block;
use ast::Expr;
use context::source::{ContentId, OwnedSource, Source, SourceSegmentHolder};
use parser::err::ParseError;
use parser::parse;

/// Represents the state of the pipeline.
#[repr(u8)]
#[derive(PartialEq, Debug, Clone, Copy, Eq)]
pub enum PipelineStatus {
    /// All the steps of the pipeline were executed successfully.
    Success,

    /// The execution of the generated bytecode exited with an error.
    ExecutionFailure,

    /// The parsing or the verification of a source failed.
    AnalysisError,

    /// A source could not be read because of an IO error.
    IoError,
}

impl PipelineStatus {
    /// Composes two statuses, by propagating error statuses.
    pub fn compose(self, other: Self) -> Self {
        match (self, other) {
            (Self::Success, other) => other,
            (Self::AnalysisError, Self::IoError) => Self::IoError, // Prefer more specific errors
            (current, _) => current,
        }
    }
}

impl Termination for PipelineStatus {
    fn report(self) -> ExitCode {
        ExitCode::from(self as u8)
    }
}

/// A collection of parse errors that are bound to a unique source.
#[derive(Debug)]
pub struct SourceAwareParseErrors {
    /// The source identifier from which the errors were generated.
    pub source: ContentId,

    /// The generated errors.
    pub errors: Vec<ParseError>,
}

/// A failure that occurred while importing a source with a [`FileImporter`].
#[derive(Debug)]
pub enum FileImportError {
    /// An IO error occurred while reading the source.
    IO { inner: io::Error, path: PathBuf },

    /// Some parse errors occurred after reading the source.
    Parse(SourceAwareParseErrors),
}

/// An importer that imports sources from the file system.
pub struct FileImporter {
    /// The root directory from which the files are read.
    root: PathBuf,

    /// a shift for local content ids
    sources_shift: usize,

    /// The imported sources, as an importer is the owner of the sources.
    sources: Vec<OwnedSource>,

    /// Paths exceptions to look for when importing a source.
    redirections: HashMap<Name, PathBuf>,

    /// The errors that occurred while importing the sources.
    ///
    /// They contains the specific errors that were masked when using the
    /// [`ASTImporter`] trait.
    errors: Vec<FileImportError>,
}

#[derive(Default)]
pub struct SourcesCache {
    cache: Vec<OwnedSource>,
}

impl SourcesCache {
    pub fn extend(&mut self, sources: Vec<OwnedSource>) {
        self.cache.extend(sources)
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }
}

impl FileImporter {
    /// Creates a new file importer that will import sources from the given
    /// root directory.
    pub fn new(sources_shift: usize, root: PathBuf) -> Self {
        Self {
            sources_shift,
            sources: vec![],
            root,
            redirections: HashMap::new(),
            errors: Vec::new(),
        }
    }

    /// Inserts a new source into the importer.
    pub fn insert<'b>(&mut self, source: OwnedSource) -> ImportResult<'b> {
        let id = self.sources.len() + self.sources_shift;
        self.sources.push(source);
        let source = self
            .sources
            .last()
            .expect("the source was just inserted")
            .as_source();
        let report = parse(source);
        if report.is_ok() {
            let expressions = unsafe {
                // SAFETY: A source is owned by the importer and is never removed.
                // A Source is the reference version to the Strings inside the OwnedSource,
                // so if the OwnedSource moves, the strings are still valid.
                // 'a is used here to disambiguate the lifetime of the source and the mutable borrow.
                std::mem::transmute::<Vec<Expr>, Vec<Expr<'b>>>(report.expr)
            };
            ImportResult::Success(Imported {
                content: ContentId(id),
                expr: Expr::Block(Block {
                    expressions,
                    segment: source.segment(),
                }),
            })
        } else {
            self.errors
                .push(FileImportError::Parse(SourceAwareParseErrors {
                    source: ContentId(id),
                    errors: report.errors,
                }));
            ImportResult::Failure
        }
    }

    /// Adds a special name to path mapping to the importer.
    pub fn add_redirection(&mut self, name: Name, path: PathBuf) {
        self.redirections.insert(name, path);
    }

    /// Gets the search path for a given name, by applying any existing redirection.
    fn get_search_path(&self, name: &Name) -> PathBuf {
        if let Some(path) = self.redirections.get(name) {
            path.clone()
        } else {
            let mut path = self.root.clone();
            path.push(name.parts().to_owned().join(MAIN_SEPARATOR_STR));
            path.with_extension("msh")
        }
    }

    pub fn take_sources(&mut self) -> Vec<OwnedSource> {
        std::mem::take(&mut self.sources)
    }
}

impl<'a> ASTImporter<'a> for FileImporter {
    fn import(&mut self, name: &Name) -> ImportResult<'a> {
        let path = self.get_search_path(name);
        match read_to_string(&path) {
            Ok(content) => self.insert(OwnedSource::new(
                content,
                path.strip_prefix(&self.root)
                    .expect("not relative")
                    .display()
                    .to_string(),
            )),
            Err(err) => {
                if err.kind() == io::ErrorKind::NotFound {
                    ImportResult::NotFound
                } else {
                    self.errors.push(FileImportError::IO { inner: err, path });
                    ImportResult::Failure
                }
            }
        }
    }
}

pub trait SourceHolder {
    /// Gets a source from the importer.
    fn get_source(&self, id: ContentId) -> Option<Source>;

    /// Lists all the contents ids that are available in the importer.
    fn list_content_ids(&self) -> Vec<ContentId>;
}

/// A trait to access errors and to get sources from an importer.
pub trait ErrorReporter {
    /// Takes the errors from the importer.
    ///
    /// This leaves the importer in a state where it has no errors.
    fn take_errors(&mut self) -> Vec<FileImportError>;
}

impl SourceHolder for SourcesCache {
    fn get_source(&self, id: ContentId) -> Option<Source> {
        self.cache.get(id.0).map(|s| s.as_source())
    }

    fn list_content_ids(&self) -> Vec<ContentId> {
        (0..self.cache.len()).map(ContentId).collect()
    }
}

impl ErrorReporter for FileImporter {
    fn take_errors(&mut self) -> Vec<FileImportError> {
        std::mem::take(&mut self.errors)
    }
}
