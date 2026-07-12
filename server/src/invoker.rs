use std::process::Command;
use crate::resolution::ResolutionContext;

#[derive(Debug)]
pub struct Diagnostic {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub severity: String,
    pub message: String,
}

fn parse_diagnostic_line(line: &str) -> Option<Diagnostic> {
    let parts: Vec<&str> = line.splitn(5, ':').collect();
    if parts.len() < 5 { return None; }
    let file     = parts[0].to_string();
    let line_num = parts[1].trim().parse::<u32>().ok()?;
    let col_num  = parts[2].trim().parse::<u32>().ok()?;
    let severity = parts[3].trim().to_string();
    let message  = parts[4].trim().to_string();
    match severity.as_str() {
        "error" | "warning" | "note" => {}
        _ => return None,
    }
    Some(Diagnostic { file, line: line_num, column: col_num, severity, message })
}

pub fn check_file(file_path: &str, ctx: &ResolutionContext) -> Vec<Diagnostic> {
    let mut cmd = Command::new("slang");

    cmd.args(["--lint-only", "--ignore-unknown-modules",
              "--diag-column", "--diag-location"]);

    for dir in &ctx.include_dirs {
        cmd.arg("-I").arg(dir);
    }

    for define in &ctx.defines {
        cmd.arg("-D").arg(define);
    }

    if let Some(ref f) = ctx.flags_file {
        cmd.arg("-F").arg(f);
    }

    if !ctx.extra_files.is_empty() {
        cmd.arg("--single-unit");
        for extra in &ctx.extra_files {
            cmd.arg(extra);
        }
    }

    cmd.arg(file_path);

    let output = cmd.output().expect("slang not found in PATH");
    let stderr  = String::from_utf8_lossy(&output.stderr);
    stderr.lines().filter_map(|l| parse_diagnostic_line(l)).collect()
}