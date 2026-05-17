#[derive(Clone, Debug, Default)]
pub struct StackModel {
    values: Vec<String>,
}

impl StackModel {
    pub fn push(&mut self, value: impl Into<String>) {
        self.values.push(value.into());
    }

    pub fn pop(&mut self) -> Option<String> {
        self.values.pop()
    }

    pub fn peek(&self) -> Option<&str> {
        self.values.last().map(String::as_str)
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
