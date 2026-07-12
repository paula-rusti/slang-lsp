use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ResolutionContext {
    pub include_dirs: Vec<String>,
    pub defines: Vec<String>,
    pub extra_files:  Vec<String>,
    pub flags_file:   Option<String>,
}

#[derive(Debug)]
pub struct CompilationDb {
    default_context: ResolutionContext,
    #[allow(dead_code)]
    per_dir: HashMap<String, ResolutionContext>,
}

impl CompilationDb {
    pub fn new(default_context: ResolutionContext) -> Self {
        Self {
            default_context,
            per_dir: HashMap::new(),
        }
    }

    pub fn get(&self, _file_path: &str) -> &ResolutionContext {
        // TODO: match against per_dir first
        &self.default_context
    }
}
