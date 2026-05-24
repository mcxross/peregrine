use serde_json::Value;
use std::{
    io::{self, BufRead, Write},
    str::FromStr,
};

const CONTENT_LENGTH_HEADER: &str = "content-length";

pub(crate) fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut content_length = None;
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read == 0 {
            return Ok(None);
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);

        if trimmed.is_empty() {
            break;
        }

        let Some((name, value)) = trimmed.split_once(':') else {
            continue;
        };

        if name.trim().eq_ignore_ascii_case(CONTENT_LENGTH_HEADER) {
            content_length = usize::from_str(value.trim()).ok();
        }
    }

    let content_length = content_length.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "LSP message missing Content-Length header",
        )
    })?;
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body)?;

    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(crate) fn write_message<W: Write + ?Sized>(writer: &mut W, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::{read_message, write_message};
    use serde_json::json;
    use std::io::{BufReader, Cursor};

    #[test]
    fn reads_content_length_framed_message() {
        let body = r#"{"jsonrpc":"2.0","method":"initialized"}"#;
        let source = format!("Content-Length: {}\r\n\r\n{body}", body.len());
        let bytes = source.into_bytes();
        let mut reader = BufReader::new(Cursor::new(bytes.as_slice()));

        assert_eq!(
            read_message(&mut reader).expect("message"),
            Some(json!({"jsonrpc":"2.0","method":"initialized"}))
        );
    }

    #[test]
    fn writes_content_length_framed_message() {
        let mut output = Vec::new();

        write_message(&mut output, &json!({"jsonrpc":"2.0","id":1})).expect("write");

        let raw = String::from_utf8(output).expect("utf8");
        assert!(raw.starts_with("Content-Length: "));
        assert_eq!(
            read_message(&mut BufReader::new(Cursor::new(raw.as_bytes()))).expect("read"),
            Some(json!({"jsonrpc":"2.0","id":1}))
        );
    }
}
