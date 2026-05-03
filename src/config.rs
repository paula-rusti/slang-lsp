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
        include_dirs.push(uvm.clone());
        extra_files.push(format!("{}/uvm_macros.svh", uvm));
    }

    let default_context = ResolutionContext {
        include_dirs,
        defines:    vec![],
        extra_files,
        flags_file,
    };

    CompilationDb::new(default_context)
}