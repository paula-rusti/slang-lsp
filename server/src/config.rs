use std::path::{Path, PathBuf};
use crate::resolution::{CompilationDb, ResolutionContext};

fn find_flags_file(root: &Path) -> Option<String> {
    let candidates = [
        ".slang/server.json",
        "slang.f",
        "project.f",
    ];

    for candidate in &candidates {
        let path: PathBuf = root.join(candidate);
        if path.exists() {
            return Some(path.to_string_lossy().into_owned());
        }
    }

    None
}

pub fn read_config(workspace_root: &str, uvm_src: Option<String>) -> CompilationDb {
    let root = Path::new(workspace_root);
    let flags_file = find_flags_file(root);

    let mut include_dirs = vec![];
    let mut extra_files  = vec![];

    if let Some(ref uvm) = uvm_src {
        let candidates = if Path::new(uvm).is_absolute() {
            vec![PathBuf::from(uvm)]
        } else {
            // Try: root/uvm  (e.g. root already includes data/)
            // Try: root/data/uvm  (e.g. root is project root, uvm is under data/)
            vec![
                root.join(uvm),
                root.join("data").join(uvm),
            ]
        };

        let uvm_path = candidates
            .into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| root.join(uvm));

        let uvm_str = uvm_path.to_string_lossy().into_owned();
        include_dirs.push(uvm_str.clone());
        extra_files.push(format!("{}/uvm_macros.svh", uvm_str));
    }

    let default_context = ResolutionContext {
        include_dirs,
        defines:    vec![],
        extra_files,
        flags_file,
    };

    CompilationDb::new(default_context)
}