mod resolution;
mod config;
mod invoker;

fn main() {
    let db = config::read_config(
        ".",
        Some("data/1800.2-2020.3.1/src".to_string()),
    );

    let file_to_analyze = "data/test_multi.sv";
    let ctx = db.get(file_to_analyze);
    let diagnostics = invoker::check_file(file_to_analyze, ctx);

    if diagnostics.is_empty() {
        println!("No errors found.")
    } else {
        for d in &diagnostics {
            println!("[{}] {}:{}:{} — {}", d.severity, d.file, d.line, d.column, d.message);
        }
    }
}