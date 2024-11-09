pub struct CodeFileOptions {
    pub line_break: String,
    pub indent: String,
}

pub struct CodeFile {
    line_break: String,
    indent_sign: String,
    indent_level: i32,
    content: String
}

impl CodeFile {
    pub fn new(options: &CodeFileOptions) -> CodeFile {
        CodeFile {
            line_break: options.line_break.to_string(),
            indent_sign: options.indent.to_string(),
            indent_level: 0,
            content: String::new()
        }
    }

    fn indent(&mut self) {
        self.indent_level += 1;
    }

    fn deindent(&mut self) {
        if self.indent_level == 0 {
            panic!("Cannot deindent, indent level is already 0")
        } else {
            self.indent_level -= 1;
        }
    }

    pub fn line(&mut self, code: &str) {
        let indent = self.indent_sign.repeat(self.indent_level as usize);
        self.content.push_str(&indent);
        self.content.push_str(code);
        self.content.push_str(&self.line_break);
    }

    pub fn blank_line(&mut self) {
        self.content.push_str(&self.line_break);
    }

    pub fn begin_indent(&mut self, code: &str) {
        self.line(code);
        self.indent();
    }

    pub fn end_indent(&mut self, code: &str) {
        self.deindent();
        self.line(code);
    }

    pub fn build_string(self) -> String {
        self.content
    }
}
