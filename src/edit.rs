//! Byte-boundary cursor editing shared by rename, create and path prompts.
pub struct Edit {
    pub text: String,
    pub cursor: usize,
}
impl Edit {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.into(),
            cursor: text.len(),
        }
    }
    pub fn key(&mut self, key: &str, clipboard: &str) {
        match key {
            "left" => {
                self.cursor = self.text[..self.cursor]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(i, _)| i)
            }
            "right" => {
                if let Some(c) = self.text[self.cursor..].chars().next() {
                    self.cursor += c.len_utf8();
                }
            }
            "home" => self.cursor = 0,
            "end" => self.cursor = self.text.len(),
            "backspace" if self.cursor > 0 => {
                let end = self.cursor;
                self.key("left", "");
                self.text.drain(self.cursor..end);
            }
            "delete" if self.cursor < self.text.len() => {
                let end = self.cursor + self.text[self.cursor..].chars().next().unwrap().len_utf8();
                self.text.drain(self.cursor..end);
            }
            "\x15" => {
                self.text.clear();
                self.cursor = 0;
            }
            "\x16" => {
                self.text.insert_str(self.cursor, clipboard);
                self.cursor += clipboard.len();
            }
            _ if key.chars().count() == 1 && !key.chars().any(char::is_control) => {
                self.text.insert_str(self.cursor, key);
                self.cursor += key.len();
            }
            _ => {}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_unicode_in_place_and_pastes_at_cursor() {
        let mut e = Edit::new("a中文.py");
        e.key("home", "");
        e.key("right", "");
        e.key("right", "");
        e.key("delete", "");
        assert_eq!(e.text, "a中.py");
        e.key("backspace", "");
        e.key("\x16", "é");
        assert_eq!(e.text, "aé.py");
        e.key("end", "");
        e.key("!", "");
        assert_eq!(e.text, "aé.py!");
    }
}
