use super::*;
/// Make depfiles escape spaces/#/backslashes and continue lines. Preserve rule
/// boundaries so repeat discoveries cannot masquerade as one merged closure.
pub(super) fn parse_depfile(bytes: &[u8]) -> Result<Vec<BTreeSet<LogicalPath>>> {
    let text = std::str::from_utf8(bytes).map_err(|e| error("DEP001", "depfile", e.to_string()))?;
    let joined = text.replace("\\\r\n", "").replace("\\\n", "");
    let mut rules = Vec::new();
    for line in joined.lines().filter(|line| !line.trim().is_empty()) {
        if rules.len() >= 256 {
            return Err(error(
                "DEP001",
                "depfile",
                "dependency rule count exceeds budget",
            ));
        }
        let rest = line
            .strip_prefix("madb-dependencies:")
            .ok_or_else(|| error("DEP001", "depfile", "unexpected depfile target"))?;
        let mut words = Vec::new();
        let mut word = String::new();
        let mut chars = rest.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\\' => word.push(
                    chars
                        .next()
                        .ok_or_else(|| error("DEP001", "depfile", "trailing depfile escape"))?,
                ),
                '$' if chars.peek() == Some(&'$') => {
                    chars.next();
                    word.push('$');
                }
                c if c.is_whitespace() => {
                    if !word.is_empty() {
                        words.push(std::mem::take(&mut word));
                    }
                }
                '#' => break,
                c => word.push(c),
            }
        }
        if !word.is_empty() {
            words.push(word)
        }
        let mut paths = BTreeSet::new();
        for word in words {
            let tail = word
                .strip_prefix("../inputs/")
                .ok_or_else(|| error("DEP003", "depfile", "dependency is outside frozen inputs"))?;
            let mut components = Vec::new();
            for part in tail.split('/') {
                match part {
                    "." | "" => {}
                    ".." => {
                        if components.pop().is_none() {
                            return Err(error(
                                "DEP003",
                                "depfile",
                                "dependency escapes frozen inputs",
                            ));
                        }
                    }
                    p => components.push(p),
                }
            }
            let path = LogicalPath::new(components.join("/"))
                .map_err(|e| error("DEP001", "depfile", e.to_string()))?;
            paths.insert(path);
            if paths.len() > 4096 {
                return Err(error(
                    "DEP001",
                    "depfile",
                    "dependency file count exceeds budget",
                ));
            }
        }
        if paths.is_empty() {
            return Err(error("DEP001", "depfile", "empty dependency rule"));
        }
        rules.push(paths);
    }
    Ok(rules)
}
