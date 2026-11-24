//!Работа с Info.plist: чтение (XML/binary) и генерация XML.

use anyhow::{Context, Result};
use plist::Value;
use std::collections::BTreeMap;
use std::path::Path;

pub type PlistDict = BTreeMap<String, Value>;

pub fn read_plist(path: &Path) -> Result<Value> {
    plist::Value::from_file(path)
        .with_context(|| format!("не удалось прочитать Info.plist: {}", path.display()))
}

pub fn read_dict(path: &Path) -> Result<PlistDict> {
    let value = read_plist(path)?;
    match value {
        Value::Dictionary(dict) => Ok(dict.into_iter().collect()),
        _ => anyhow::bail!("Info.plist должен быть словарём: {}", path.display()),
    }
}

fn write_value(value: &Value, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match value {
        Value::String(s) => {
            out.push_str("<string>");
            out.push_str(&xml_escape(s));
            out.push_str("</string>");
        }
        Value::Integer(i) => {
            out.push_str("<integer>");
            out.push_str(&i.to_string());
            out.push_str("</integer>");
        }
        Value::Real(r) => {
            out.push_str("<real>");
            out.push_str(&r.to_string());
            out.push_str("</real>");
        }
        Value::Boolean(b) => out.push_str(if *b { "<true/>" } else { "<false/>" }),
        Value::Array(items) => {
            out.push_str("<array>\n");
            for item in items {
                out.push_str(&"  ".repeat(indent + 1));
                write_value(item, indent + 1, out);
                out.push('\n');
            }
            out.push_str(&pad);
            out.push_str("</array>");
        }
        Value::Dictionary(dict) => {
            out.push_str("<dict>\n");
            for (k, v) in dict {
                out.push_str(&"  ".repeat(indent + 1));
                out.push_str("<key>");
                out.push_str(&xml_escape(k));
                out.push_str("</key>\n");
                out.push_str(&"  ".repeat(indent + 1));
                write_value(v, indent + 1, out);
                out.push('\n');
            }
            out.push_str(&pad);
            out.push_str("</dict>");
        }
        Value::Data(d) => {
            out.push_str("<data>");
            out.push_str(&base64_encode(d));
            out.push_str("</data>");
        }
        Value::Date(d) => {
            out.push_str("<date>");
            out.push_str(&format!("{d:?}"));
            out.push_str("</date>");
        }
        _ => out.push_str("<string></string>"),
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

pub fn to_xml_string(dict: &PlistDict) -> String {
    let value = Value::Dictionary(dict.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n",
    );
    write_value(&value, 1, &mut out);
    out.push_str("\n</plist>\n");
    out
}

pub fn str_value(s: impl Into<String>) -> Value {
    Value::String(s.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generates_xml() {
        let mut d = PlistDict::new();
        d.insert("CFBundleName".into(), str_value("MyApp"));
        d.insert("CFBundleVersion".into(), str_value("1"));
        let xml = to_xml_string(&d);
        assert!(xml.contains("<key>CFBundleName</key>"));
        assert!(xml.contains("<string>MyApp</string>"));
        let parsed = plist::Value::from_reader_xml(xml.as_bytes()).unwrap();
        assert!(parsed.as_dictionary().is_some());
    }
}
