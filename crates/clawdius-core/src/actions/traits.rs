use anyhow::Result;

pub struct ActionContext {
    pub document: String,
    pub language: String,
    pub selection: Option<String>,
    pub position: (usize, usize),
}

pub struct ActionEdit {
    pub title: String,
    pub edits: Vec<TextEdit>,
}

pub struct TextEdit {
    pub start: (usize, usize),
    pub end: (usize, usize),
    pub new_text: String,
}

pub trait CodeAction: Send + Sync {
    fn name(&self) -> &str;
    fn is_applicable(&self, context: &ActionContext) -> bool;
    fn execute(&self, context: &ActionContext) -> Result<ActionEdit>;
}
