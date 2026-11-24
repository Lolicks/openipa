//! Парсер формата OpenStep (NeXTstep plist), которым написан project.pbxproj.
//! Поддерживает словари, массивы, строки (кавычки и «голые» токены),
//! data-литералы `<...>` и комментарии `/* */` и `//`.

use anyhow::{bail, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Dict(Vec<(String, Value)>),
    Array(Vec<Value>),
    Data(Vec<u8>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_dict(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Dict(d) => Some(d),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_dict()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }
}

pub struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.pos += 1;
        Some(c)
    }

    fn skip_ws_comments(&mut self) {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\t') | Some(b'\r') | Some(b'\n') | Some(0x0c) => {
                    self.pos += 1;
                }
                Some(b'/') if self.src.get(self.pos + 1) == Some(&b'*') => {
                    self.pos += 2;
                    while self.pos + 1 < self.src.len() {
                        if self.src[self.pos] == b'*' && self.src[self.pos + 1] == b'/' {
                            self.pos += 2;
                            break;
                        }
                        self.pos += 1;
                    }
                }
                Some(b'/') if self.src.get(self.pos + 1) == Some(&b'/') => {
                    self.pos += 2;
                    while let Some(c) = self.peek() {
                        if c == b'\n' {
                            break;
                        }
                        self.pos += 1;
                    }
                }
                _ => break,
            }
        }
    }

    pub fn parse(mut self) -> Result<Value> {
        self.skip_ws_comments();
        // Xcode кладёт в начало файл "// !$*UTF8*$!" — комментарий, уже пропущен.
        let v = self.parse_value()?;
        self.skip_ws_comments();
        if self.pos != self.src.len() {
            bail!(
                "неожиданные данные в pbxproj на байте {} ({} байт осталось)",
                self.pos,
                self.src.len() - self.pos
            );
        }
        Ok(v)
    }

    fn parse_value(&mut self) -> Result<Value> {
        self.skip_ws_comments();
        match self.peek() {
            Some(b'{') => self.parse_dict(),
            Some(b'(') => self.parse_array(),
            Some(b'<') => self.parse_data(),
            Some(b'"') => Ok(Value::String(self.parse_quoted()?)),
            Some(_) => Ok(Value::String(self.parse_bare()?)),
            None => bail!("неожиданный конец pbxproj"),
        }
    }

    fn parse_dict(&mut self) -> Result<Value> {
        self.bump(); // {
        let mut entries = Vec::new();
        loop {
            self.skip_ws_comments();
            match self.peek() {
                Some(b'}') => {
                    self.bump();
                    break;
                }
                None => bail!("незакрытый словарь в pbxproj"),
                _ => {}
            }
            let key = match self.peek() {
                Some(b'"') => self.parse_quoted()?,
                Some(_) => self.parse_bare()?,
                None => bail!("нет ключа в словаре pbxproj"),
            };
            self.skip_ws_comments();
            if self.bump() != Some(b'=') {
                bail!("ожидался '=' после ключа '{}'", key);
            }
            let value = self.parse_value()?;
            self.skip_ws_comments();
            if self.bump() != Some(b';') {
                bail!("ожидался ';' после значения ключа '{}'", key);
            }
            entries.push((key, value));
        }
        Ok(Value::Dict(entries))
    }

    fn parse_array(&mut self) -> Result<Value> {
        self.bump(); // (
        let mut items = Vec::new();
        loop {
            self.skip_ws_comments();
            match self.peek() {
                Some(b')') => {
                    self.bump();
                    break;
                }
                None => bail!("незакрытый массив в pbxproj"),
                _ => {
                    items.push(self.parse_value()?);
                    self.skip_ws_comments();
                    if self.peek() == Some(b',') {
                        self.bump();
                    }
                }
            }
        }
        Ok(Value::Array(items))
    }

    fn parse_data(&mut self) -> Result<Value> {
        self.bump(); // <
        let mut hex = String::new();
        while let Some(c) = self.peek() {
            if c == b'>' {
                self.bump();
                break;
            }
            if c.is_ascii_hexdigit() {
                hex.push(c as char);
            }
            self.pos += 1;
        }
        let bytes = (0..hex.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
            .collect();
        Ok(Value::Data(bytes))
    }

    fn parse_quoted(&mut self) -> Result<String> {
        self.bump(); // "
        let mut out = String::new();
        while let Some(c) = self.bump() {
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self.bump().unwrap_or(b'\\');
                    match e {
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'U' => {
                            let mut code = String::new();
                            for _ in 0..4 {
                                if let Some(h) = self.bump() {
                                    code.push(h as char);
                                }
                            }
                            if let Ok(cp) = u32::from_str_radix(&code, 16) {
                                if let Some(ch) = char::from_u32(cp) {
                                    out.push(ch);
                                }
                            }
                        }
                        other => out.push(other as char),
                    }
                }
                _ => out.push(c as char),
            }
        }
        bail!("незакрытая строка в pbxproj")
    }

    fn parse_bare(&mut self) -> Result<String> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace()
                || matches!(
                    c,
                    b'=' | b';' | b',' | b'{' | b'}' | b'(' | b')' | b'"' | b'<'
                )
            {
                break;
            }
            self.pos += 1;
        }
        if self.pos == start {
            bail!("пустой токен в pbxproj на байте {}", self.pos);
        }
        Ok(String::from_utf8_lossy(&self.src[start..self.pos]).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_dict() {
        let src = r#"// !$*UTF8*$!
{ archiveVersion = 1; objects = { A = { isa = PBXFileReference; path = "a.swift"; }; }; rootObject = A; }"#;
        let v = Parser::new(src).parse().unwrap();
        assert_eq!(v.get_str("archiveVersion"), Some("1"));
        let objects = v.get("objects").unwrap();
        let a = objects.get("A").unwrap();
        assert_eq!(a.get_str("isa"), Some("PBXFileReference"));
        assert_eq!(a.get_str("path"), Some("a.swift"));
        assert_eq!(v.get_str("rootObject"), Some("A"));
    }

    #[test]
    fn parses_array_and_comments() {
        let src = r#"{ phases = ( /* one */ A, B /* two */, "C D" ); }"#;
        let v = Parser::new(src).parse().unwrap();
        let arr = v.get("phases").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0].as_str(), Some("A"));
        assert_eq!(arr[2].as_str(), Some("C D"));
    }
}
