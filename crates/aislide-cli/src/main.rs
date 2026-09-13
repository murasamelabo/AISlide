use aislide_core::{Error, Result, execute_request, protocol::MAX_REQUEST_BYTES, report::{ReportInput, compile_report}, pptx::{export_pptx, inspect_pptx}};
use std::io::{Read, Write};
use std::path::Path;

fn main() {
    if let Err(error) = run() {
        eprintln!("{}", serde_json::json!({"error":error.to_string()}));
        std::process::exit(1);
    }
}

fn bounded_read(reader: impl Read, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit { return Err(Error::Limit("input file budget".into())); }
    Ok(bytes)
}

fn json_bytes(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)
}

fn print_json(value: &serde_json::Value) -> Result<()> {
    let mut output = std::io::stdout().lock();
    serde_json::to_writer(&mut output, value)?;
    writeln!(output)?;
    Ok(())
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map_or("request", String::as_str) {
        "request" if args.len() <= 1 => {
            let bytes = bounded_read(std::io::stdin().lock(), MAX_REQUEST_BYTES)?;
            let response = execute_request(serde_json::from_slice(json_bytes(&bytes))?)?;
            print_json(&response)
        }
        "sample" if args.len() == 1 => print_json(&execute_request(serde_json::json!({"op":"sample"}))?),
        "generate" if args.len() == 3 => {
            let input = bounded_read(std::fs::File::open(&args[1])?, MAX_REQUEST_BYTES)?;
            let report: ReportInput = serde_json::from_slice(json_bytes(&input))?;
            let compiled = compile_report(&report)?;
            let bytes = export_pptx(&compiled.deck)?;
            let destination = Path::new(&args[2]);
            let parent = destination.parent().filter(|value| !value.as_os_str().is_empty()).unwrap_or(Path::new("."));
            let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
            temporary.write_all(&bytes)?;
            temporary.as_file().sync_all()?;
            temporary.persist_noclobber(destination).map_err(|error| Error::Io(error.error))?;
            print_json(&serde_json::json!({"path":destination,"slides":compiled.deck.slides.len(),"bytes":bytes.len()}))
        }
        "inspect" if args.len() == 2 => {
            let bytes = bounded_read(std::fs::File::open(&args[1])?, aislide_core::package::MAX_ARCHIVE_BYTES)?;
            print_json(&serde_json::to_value(inspect_pptx(bytes)?)?)
        }
        "--help" | "help" => {
            println!("AISlide\n  request                       JSON stdin -> JSON stdout\n  sample                        print the synthetic report input\n  generate input.json out.pptx   create a new native PPTX (no overwrite)\n  inspect file.pptx             inspect simple top-level text runs");
            Ok(())
        }
        _ => Err(Error::Invalid("unknown command or arguments; use --help".into())),
    }
}