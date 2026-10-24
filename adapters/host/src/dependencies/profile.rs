use madb_app::dependencies::DependencyRequest;
pub(super) fn supported_request(r: &DependencyRequest) -> Option<String> {
    if ![
        "arm64-apple-macos11.0",
        "x86_64-apple-macos10.15",
        "aarch64-unknown-linux-gnu",
        "x86_64-unknown-linux-gnu",
    ]
    .contains(&r.target.as_str())
    {
        return Some("target is outside the explicit native preparation target set".into());
    }

    if r.sources.is_empty()
        || r.sources.len() > 128
        || r.include_directories.len() > 128
        || r.arguments.len() > 256
        || r.compiler.size > 256 * 1024 * 1024
    {
        return Some("request exceeds supported bounds".into());
    }
    if r.sources.iter().any(|s| !s.as_str().contains('/')) {
        return Some("sources require an explicit workspace-relative parent directory".into());
    }
    if !r.arguments.iter().any(|arg| arg == "-x")
        && r.sources.iter().any(|path| {
            !path
                .as_str()
                .rsplit_once('.')
                .is_some_and(|(_, extension)| {
                    ["c", "h", "C", "cc", "cpp", "cxx", "hh", "hpp", "hxx"].contains(&extension)
                })
        })
    {
        return Some("source suffix requires an explicit supported -x c/c++ language".into());
    }
    if r.environment.iter().any(|(k, v)| {
        !["LANG", "LC_ALL", "SOURCE_DATE_EPOCH"].contains(&k.as_str())
            || v.len() > 4096
            || v.contains('\0')
    }) {
        return Some("environment is outside the explicit locale/time whitelist".into());
    }
    let mut args = r.arguments.iter();
    while let Some(arg) = args.next() {
        if arg.len() > 4096 || arg.contains('\0') {
            return Some("invalid compiler argument".into());
        }
        let accepted = if arg == "-x" {
            args.next().is_some_and(|x| x == "c" || x == "c++")
        } else if arg.starts_with("-D") && arg.len() > 2 {
            !arg.contains(['#', '"', '\\', '/', '\n', '\r']) && !arg.contains("__has_")
        } else if arg.starts_with("-U") && arg.len() > 2 {
            arg[2..]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        } else if let Some(std) = arg.strip_prefix("-std=") {
            [
                "c89", "c90", "c99", "c11", "c17", "c18", "c23", "gnu89", "gnu99", "gnu11",
                "gnu17", "c++11", "c++14", "c++17", "c++20", "c++23", "gnu++17", "gnu++20",
            ]
            .contains(&std)
        } else {
            false
        };
        if !accepted {
            return Some(format!(
                "compiler option is not covered by the freestanding dependency provider: {arg}"
            ));
        }
    }
    None
}
pub(super) fn native_image(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x7fELF")
        || bytes.get(..4).is_some_and(|b| {
            [
                &[0xcf, 0xfa, 0xed, 0xfe][..],
                &[0xfe, 0xed, 0xfa, 0xcf][..],
                &[0xca, 0xfe, 0xba, 0xbe][..],
                &[0xbe, 0xba, 0xfe, 0xca][..],
            ]
            .contains(&b)
        })
}

// Conservative syntax profile: dynamic includes and alternate preprocessor
// spellings need a different provider. Rejecting them never enables cache use.
pub(super) fn supported_source(bytes: &[u8]) -> Option<String> {
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return Some("non-UTF8 source outside restricted profile".into()),
    };
    for line in text.lines() {
        if let Some((_, tail)) = line.rsplit_once('\\')
            && !tail.is_empty()
            && tail.chars().all(|c| c.is_ascii_whitespace())
        {
            return Some("nonstandard whitespace line continuation is unsupported".into());
        }
    }
    let text = text.replace("\\\r\n", "").replace("\\\n", "");
    let text = strip_comments(&text);
    if [
        "??",
        "%:",
        "##",
        "_Pragma",
        "__pragma",
        "#pragma",
        "#include_next",
        "#embed",
        "__has_embed",
    ]
    .iter()
    .any(|s| text.contains(s))
    {
        return Some("unsupported preprocessor escape or dynamic directive".into());
    }
    for line in text.lines() {
        let line = line.trim_start();
        if let Some(directive) = line.strip_prefix('#') {
            let directive = directive.trim_start();
            let name = directive
                .split(|c: char| !c.is_ascii_alphabetic() && c != '_')
                .next()
                .unwrap_or("");
            if ![
                "include", "define", "undef", "if", "ifdef", "ifndef", "elif", "else", "endif",
                "error", "warning", "line", "",
            ]
            .contains(&name)
            {
                return Some("unknown preprocessor directive is outside restricted profile".into());
            }
            if let Some(rest) = directive.strip_prefix("include") {
                if !rest.starts_with(char::is_whitespace) {
                    return Some("unsupported include directive".into());
                }
                let rest = rest.trim_start();
                let (close, rest) = match rest.as_bytes().first() {
                    Some(b'"') => ('"', &rest[1..]),
                    Some(b'<') => ('>', &rest[1..]),
                    _ => {
                        return Some("computed includes are outside the restricted profile".into());
                    }
                };
                let Some((path, _)) = rest.split_once(close) else {
                    return Some("unterminated include path".into());
                };
                if !safe_include(path) {
                    return Some("include escapes the explicit search domain".into());
                }
            } else if directive.starts_with("pragma") || directive.starts_with("embed") {
                return Some("unsupported source directive".into());
            }
        }
    }
    // Even unused __has_include probes are hidden filesystem reads. Require
    // literal names and reject traversal/absolute strings everywhere so macro
    // aliases cannot introduce an unobserved search condition.
    for tail in text.split("__has_include").skip(1) {
        let Some(tail) = tail.trim_start().strip_prefix('(') else {
            return Some("aliased include probes are unsupported".into());
        };
        let tail = tail.trim_start();
        let (close, tail) = match tail.as_bytes().first() {
            Some(b'"') => ('"', &tail[1..]),
            Some(b'<') => ('>', &tail[1..]),
            _ => return Some("computed include probes are unsupported".into()),
        };
        let Some((path, tail)) = tail.split_once(close) else {
            return Some("unterminated include probe".into());
        };
        if !safe_include(path) || !tail.trim_start().starts_with(')') {
            return Some("include probe escapes the explicit search domain".into());
        }
    }
    for (index, string) in text.split('"').enumerate() {
        if index % 2 == 1
            && (string.starts_with('/') || string.contains("../") || string.contains("..\\"))
        {
            return Some(
                "absolute/traversing strings are outside the restricted source profile".into(),
            );
        }
    }
    None
}
// Remove comments before looking for directives. Whitespace is preserved so
// comments cannot hide a directive keyword or join formerly separate tokens.
fn strip_comments(text: &str) -> String {
    let mut chars = text.chars().peekable();
    let mut out = String::with_capacity(text.len());
    let mut quote = None;
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            out.push(c);
            if c == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next)
                }
            } else if c == q {
                quote = None
            }
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            out.push(c);
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            out.push(' ');
            while let Some(c) = chars.next() {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
                if c == '\n' {
                    out.push('\n')
                }
            }
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for c in chars.by_ref() {
                if c == '\n' {
                    out.push('\n');
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}
fn safe_include(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.split('/').any(|part| part == ".." || part.is_empty())
}
