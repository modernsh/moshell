use context::source::Source;
use miette::{Diagnostic, GraphicalReportHandler, SourceSpan};
use parser::err::ParseErrorKind;
use parser::parse;
use std::fmt::Display;
use std::io::{self, BufRead};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
struct FormattedError<'s> {
    #[source_code]
    src: &'s Source<'s>,
    #[label("Here")]
    cursor: SourceSpan,
    #[label("Start")]
    related: Option<SourceSpan>,
    message: String,
    #[help]
    help: Option<String>,
}

impl Display for FormattedError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    let handler = GraphicalReportHandler::default();

    while let Some(line) = lines.next() {
        let line = line?;
        let source = Source::new(&line, "stdin");
        let report = parse(source.clone());
        let errors = report
            .errors
            .into_iter()
            .map(|err| FormattedError {
                src: &source,
                cursor: err.position.into(),
                related: match &err.kind {
                    ParseErrorKind::Unpaired(pos) => Some(pos.clone().into()),
                    _ => None,
                },
                message: err.message,
                help: match &err.kind {
                    ParseErrorKind::Excepted(excepted) => Some(format!("Excepted: {:?}", excepted)),
                    _ => None,
                },
            })
            .collect::<Vec<_>>();

        if errors.is_empty() {
            println!("{:?}", report.expr);
            continue;
        }
        let mut msg = String::new();
        for err in &errors {
            if let Err(fmt_err) = handler.render_report(&mut msg, err) {
                eprintln!("{fmt_err}");
            } else {
                eprintln!("{msg}");
            }
            msg.clear();
        }
    }

    Ok(())
}
