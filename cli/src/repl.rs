use crate::cli::{display_exprs, display_tokens, Configuration};
use crate::formatted_diagnostic::render_diagnostic;
use crate::formatted_parse_error::render_parse_error;
use analyzer::engine::Engine;
use analyzer::importer::ASTImporter;
use analyzer::name::Name;
use analyzer::relations::Relations;
use ast::group::Block;
use ast::Expr;
use context::source::{OwnedSource, Source};
use lexer::lexer::lex;
use miette::GraphicalReportHandler;
use parser::parse;
use rustyline::config::Configurer;
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use rustyline::{
    Cmd, ColorMode, DefaultEditor, Editor, Event, EventHandler, KeyCode, KeyEvent, Modifiers,
};
use std::collections::HashMap;
use std::process::exit;

type REPLEditor = Editor<(), DefaultHistory>;

#[derive(Default)]
struct REPLImporter<'a> {
    #[allow(clippy::vec_box)]
    // Box is used to ensure that the reference behind is still valid after vector's realloc
    sources: Vec<Box<OwnedSource>>,
    imported_modules: HashMap<Name, Expr<'a>>,
}

impl<'a, const N: usize> From<[(Name, Expr<'a>); N]> for REPLImporter<'a> {
    fn from(value: [(Name, Expr<'a>); N]) -> Self {
        Self {
            imported_modules: HashMap::from(value),
            sources: Vec::new(),
        }
    }
}

impl<'a> ASTImporter<'a> for REPLImporter<'a> {
    fn import(&mut self, name: &Name) -> Option<Expr<'a>> {
        self.imported_modules.get(name).cloned()
    }
}

impl<'a> REPLImporter<'a> {
    pub fn take_source(&mut self, source: OwnedSource) -> Source<'a> {
        self.sources.push(Box::new(source));
        let src = self.sources[self.sources.len() - 1].as_source();
        unsafe {
            // SAFETY: The sources will never be removed from the self.sources vector as the REPLImporter's
            // role is to be the owner of the user's sources.
            // The reference behind Box does not change and is valid for the lifetime of the importer.
            std::mem::transmute::<Source, Source<'a>>(src)
        }
    }
}

/// Indefinitely prompts a new expression to the stdin,
/// displaying back the errors if any and the formed AST
pub fn repl(config: Configuration, handler: GraphicalReportHandler) {
    let mut editor: REPLEditor =
        DefaultEditor::new().expect("unable to instantiate terminal editor");
    editor.set_color_mode(ColorMode::Enabled);
    editor.set_history_ignore_dups(true).unwrap();
    editor.set_history_ignore_space(true);
    editor.bind_sequence(
        Event::KeySeq(vec![KeyEvent(KeyCode::Char('u'), Modifiers::ALT)]),
        EventHandler::from(Cmd::Undo(1)),
    );

    let mut engine = Engine::default();
    let mut relations = Relations::default();
    let mut importer = REPLImporter::default();

    loop {
        let source = parse_input(&mut editor);
        handle_source(
            source,
            &config,
            &mut engine,
            &mut importer,
            &mut relations,
            &handler,
        );
    }
}

fn strip_indent(line: &mut String) -> String {
    let mut indent = String::new();
    let line_clone = line.to_string();
    let chars = line_clone.chars();
    for c in chars {
        if !c.is_whitespace() {
            return indent;
        }
        line.remove(0);
        indent.push(c);
    }
    indent
}

/// Parses stdin until the user's input forms a source code with no unclosed delimiters
/// and return the source.
fn parse_input(editor: &mut REPLEditor) -> OwnedSource {
    let mut content = String::new();
    let mut prompt_prefix = "=> ".to_string();
    let mut indent_prefix = String::new();
    loop {
        let line = editor.readline_with_initial(&prompt_prefix, (&indent_prefix, ""));
        let mut line = match line {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => exit(1),
            e => e.expect("error when reading next line from editor"),
        };

        editor
            .add_history_entry(line.clone())
            .expect("terminal has no history");

        indent_prefix = strip_indent(&mut line);

        content.push_str(&line);
        if line.ends_with('\\') {
            content.push('\n');
            prompt_prefix = "-> ".to_string();
            continue;
        }

        let source = Source::new(&content, "stdin");
        let report = parse(source);
        if let Some(last) = report.delimiter_stack.last() {
            content.push('\n');
            prompt_prefix = format!("{}> ", last.str().unwrap());
            continue; // Silently ignore incomplete input
        }

        return OwnedSource::new(source.source.to_string(), source.name.to_string());
    }
}

/// Parses and display errors / diagnostics coming from the given source.
/// Returning true if the source had at least one error or diagnostic.
fn handle_source<'a, 'e>(
    source: OwnedSource,
    config: &Configuration,
    engine: &mut Engine<'e>,
    importer: &mut REPLImporter<'e>,
    relations: &mut Relations,
    handler: &GraphicalReportHandler,
) -> bool {
    let source = importer.take_source(source);
    let name = Name::new(&source.name);

    if config.lexer_visualisation {
        display_tokens(lex(source.source))
    }

    let report = parse(source);

    let source = source;
    let errors: Vec<_> = report.errors;

    if !errors.is_empty() {
        for error in errors {
            let str = render_parse_error(error, handler, source)
                .expect("IO error when reporting diagnostics");
            eprintln!("{str}")
        }
        return true;
    }

    if config.parser_visualization {
        display_exprs(&report.expr)
    }

    let expr = Expr::Block(Block {
        expressions: report.expr,
        segment: 0..0,
    });

    importer.imported_modules.insert(name.clone(), expr);

    let diagnostics = analyzer::make_full_resolution(name, importer, engine, relations);

    let had_errors = !diagnostics.is_empty();
    for diagnostic in diagnostics {
        let str = render_diagnostic(source, diagnostic, handler)
            .expect("IO errors when reporting diagnostic");
        eprintln!("{str}")
    }
    had_errors
}
