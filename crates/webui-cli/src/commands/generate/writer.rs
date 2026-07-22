// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

pub(super) struct CodeWriter {
    output: String,
    indent: usize,
    indent_unit: &'static str,
    line_start: bool,
}

impl CodeWriter {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            output: String::with_capacity(capacity),
            indent: 0,
            indent_unit: "    ",
            line_start: true,
        }
    }

    pub(super) fn with_indent(capacity: usize, indent_unit: &'static str) -> Self {
        Self {
            output: String::with_capacity(capacity),
            indent: 0,
            indent_unit,
            line_start: true,
        }
    }

    pub(super) fn push(&mut self, value: &str) {
        if self.line_start {
            for _ in 0..self.indent {
                self.output.push_str(self.indent_unit);
            }
            self.line_start = false;
        }
        self.output.push_str(value);
    }

    pub(super) fn push_char(&mut self, value: char) {
        if self.line_start {
            for _ in 0..self.indent {
                self.output.push_str(self.indent_unit);
            }
            self.line_start = false;
        }
        self.output.push(value);
    }

    pub(super) fn line(&mut self, value: &str) {
        self.push(value);
        self.newline();
    }

    pub(super) fn newline(&mut self) {
        self.output.push('\n');
        self.line_start = true;
    }

    pub(super) fn blank_line(&mut self) {
        if !self.line_start {
            self.newline();
        }
        self.output.push('\n');
    }

    pub(super) fn indent(&mut self) {
        self.indent += 1;
    }

    pub(super) fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    pub(super) fn finish(mut self) -> String {
        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        self.output
    }
}
