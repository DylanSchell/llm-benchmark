//! Minimal JSON parser (enough for Day 12).
//! Supports objects, arrays, numbers, strings, booleans, null.

#[derive(Debug, Clone)]
pub enum Json {
    Null,
    #[allow(dead_code)]
    Bool(bool),
    Num(i64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

pub fn parse(input: &str) -> Result<Json, String> {
    let b = input.trim().as_bytes();
    let mut pos = 0;
    let v = parse_value(b, &mut pos)?;
    Ok(v)
}

fn parse_value(b: &[u8], pos: &mut usize) -> Result<Json, String> {
    skip_ws(b, pos);
    if *pos >= b.len() {
        return Err("unexpected end".into());
    }
    match b[*pos] {
        b'{' => parse_object(b, pos),
        b'[' => parse_array(b, pos),
        b'"' => Ok(Json::Str(parse_string(b, pos)?)),
        b't' => {
            expect(b, pos, "true")?;
            Ok(Json::Bool(true))
        }
        b'f' => {
            expect(b, pos, "false")?;
            Ok(Json::Bool(false))
        }
        b'n' => {
            expect(b, pos, "null")?;
            Ok(Json::Null)
        }
        _ => Ok(Json::Num(parse_number(b, pos)?)),
    }
}

fn parse_object(b: &[u8], pos: &mut usize) -> Result<Json, String> {
    *pos += 1; // {
    let mut out = Vec::new();
    skip_ws(b, pos);
    if *pos < b.len() && b[*pos] == b'}' {
        *pos += 1;
        return Ok(Json::Object(out));
    }
    loop {
        skip_ws(b, pos);
        let key = parse_string(b, pos)?;
        skip_ws(b, pos);
        if *pos >= b.len() || b[*pos] != b':' {
            return Err("expected :".into());
        }
        *pos += 1;
        let val = parse_value(b, pos)?;
        out.push((key, val));
        skip_ws(b, pos);
        if *pos < b.len() && b[*pos] == b',' {
            *pos += 1;
            continue;
        }
        if *pos < b.len() && b[*pos] == b'}' {
            *pos += 1;
            break;
        }
        return Err("expected , or }".into());
    }
    Ok(Json::Object(out))
}

fn parse_array(b: &[u8], pos: &mut usize) -> Result<Json, String> {
    *pos += 1; // [
    let mut out = Vec::new();
    skip_ws(b, pos);
    if *pos < b.len() && b[*pos] == b']' {
        *pos += 1;
        return Ok(Json::Array(out));
    }
    loop {
        let val = parse_value(b, pos)?;
        out.push(val);
        skip_ws(b, pos);
        if *pos < b.len() && b[*pos] == b',' {
            *pos += 1;
            continue;
        }
        if *pos < b.len() && b[*pos] == b']' {
            *pos += 1;
            break;
        }
        return Err("expected , or ]".into());
    }
    Ok(Json::Array(out))
}

fn parse_string(b: &[u8], pos: &mut usize) -> Result<String, String> {
    if b[*pos] != b'"' {
        return Err("expected quote".into());
    }
    *pos += 1;
    let mut out = String::new();
    while *pos < b.len() {
        let c = b[*pos];
        *pos += 1;
        if c == b'"' {
            return Ok(out);
        }
        if c == b'\\' {
            if *pos >= b.len() {
                return Err("bad escape".into());
            }
            let e = b[*pos];
            *pos += 1;
            out.push(match e {
                b'n' => '\n',
                b't' => '\t',
                b'r' => '\r',
                b'b' => '\u{0008}',
                b'f' => '\u{000C}',
                b'"' => '"',
                b'\\' => '\\',
                b'/' => '/',
                b'u' => {
                    if *pos + 4 > b.len() {
                        return Err("bad unicode".into());
                    }
                    let hex = std::str::from_utf8(&b[*pos..*pos + 4]).unwrap();
                    let cp = u32::from_str_radix(hex, 16).map_err(|_| "bad unicode")?;
                    *pos += 4;
                    char::from_u32(cp).unwrap_or('\u{FFFD}')
                }
                _ => e as char,
            });
        } else {
            out.push(c as char);
        }
    }
    Err("unterminated string".into())
}

fn parse_number(b: &[u8], pos: &mut usize) -> Result<i64, String> {
    let start = *pos;
    if *pos < b.len() && b[*pos] == b'-' {
        *pos += 1;
    }
    while *pos < b.len() && b[*pos].is_ascii_digit() {
        *pos += 1;
    }
    let s = std::str::from_utf8(&b[start..*pos]).map_err(|_| "bad number")?;
    s.parse().map_err(|_| "bad number".into())
}

fn skip_ws(b: &[u8], pos: &mut usize) {
    while *pos < b.len() && (b[*pos] as char).is_whitespace() {
        *pos += 1;
    }
}

fn expect(b: &[u8], pos: &mut usize, lit: &str) -> Result<(), String> {
    if b[*pos..].starts_with(lit.as_bytes()) {
        *pos += lit.len();
        Ok(())
    } else {
        Err(format!("expected {}", lit))
    }
}
