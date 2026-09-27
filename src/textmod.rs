//! Text mod (BLCMM / FilterTool) parsing and merging.
//!
//! Text Mod Loader locks every other hotfix-using mod once the first one has
//! run, so Vaulter's community patch is built by merging several
//! upstream mods into a single offline BLCMM file on the user's machine.
//! The output format follows OpenBLCMM's `PatchIO` (offline mode):
//! the XML-ish tree for editors, then `#Commands:` (plain `set` lines) and
//! `#Hotfixes:` (one Spark service holding every hotfix, Gearbox's own
//! official hotfixes first).

use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HotfixKind {
    /// `SparkPatchEntry`: applies globally.
    Patch,
    /// `SparkLevelPatchEntry`: applies when the level loads ("" = any level).
    Level(String),
    /// `SparkOnDemandPatchEntry`: applies when the package streams in.
    OnDemand(String),
}

impl HotfixKind {
    fn key_prefix(&self) -> &'static str {
        match self {
            HotfixKind::Patch => "SparkPatchEntry",
            HotfixKind::Level(_) => "SparkLevelPatchEntry",
            HotfixKind::OnDemand(_) => "SparkOnDemandPatchEntry",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Category { name: String, children: Vec<Node> },
    Comment(String),
    /// An enabled `set`/`set_cmp` line.
    Code(String),
    Hotfix { name: String, kind: HotfixKind, codes: Vec<String> },
}

/// Text mods are ANSI (the game's `exec` reads bytes, not UTF-8). Decode
/// UTF-8 when it's valid, otherwise treat every byte as Latin-1.
pub fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// Encodes back to single-byte Latin-1; anything outside it becomes `?`.
pub fn encode(text: &str) -> Vec<u8> {
    text.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect()
}

/// Reads `name="value"` from a tag, honouring BLCMM's `\"` escaping.
fn attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let mut out = String::new();
    let mut chars = tag[start..].chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            }
            '"' => return Some(out),
            c => out.push(c),
        }
    }
    None
}

/// A `<code>` line is active if it has no profile list or lists "default".
fn parse_code(line: &str) -> Option<(bool, String)> {
    let rest = line.strip_prefix("<code")?;
    let close = rest.find('>')?;
    let tag = &rest[..close];
    let body = rest[close + 1..].strip_suffix("</code>").unwrap_or(&rest[close + 1..]);
    let enabled = match attr(tag, "profiles") {
        None => true,
        Some(p) => p.split(',').any(|p| p.trim() == "default"),
    };
    Some((enabled, body.trim().to_string()))
}

/// Parses a BLCMM file into its category tree (enabled codes only), or a
/// flat FilterTool/plain file into a single list of `set` codes.
pub fn parse(text: &str) -> Vec<Node> {
    if !text.trim_start().starts_with("<BLCMM") {
        return text
            .lines()
            .map(str::trim)
            .filter(|l| {
                let lower = l.to_ascii_lowercase();
                (lower.starts_with("set ") || lower.starts_with("set_cmp ")) && !lower.contains("transient.sparkserviceconfiguration")
                    && !lower.contains("transient.gearboxaccountdata")
            })
            .map(|l| Node::Code(l.to_string()))
            .collect();
    }

    // Stack of open categories: (name, children).
    let mut stack: Vec<(String, Vec<Node>)> = vec![(String::new(), Vec::new())];
    let mut hotfix: Option<(String, HotfixKind, Vec<String>)> = None;
    let mut in_body = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line == "</BLCMM>" {
            break;
        }
        if line == "<body>" {
            in_body = true;
            continue;
        }
        if !in_body || line == "</body>" {
            continue;
        }
        if line.starts_with("<category") {
            let name = attr(line, "name").unwrap_or_default();
            if line.ends_with("/>") {
                stack.last_mut().unwrap().1.push(Node::Category { name, children: Vec::new() });
            } else {
                stack.push((name, Vec::new()));
            }
        } else if line == "</category>" {
            if stack.len() > 1 {
                let (name, children) = stack.pop().unwrap();
                stack.last_mut().unwrap().1.push(Node::Category { name, children });
            }
        } else if line.starts_with("<hotfix") {
            let name = attr(line, "name").unwrap_or_default();
            let kind = if let Some(level) = attr(line, "level") {
                HotfixKind::Level(if level == "None" { String::new() } else { level })
            } else if let Some(package) = attr(line, "package") {
                HotfixKind::OnDemand(package)
            } else {
                HotfixKind::Patch
            };
            hotfix = Some((name, kind, Vec::new()));
        } else if line == "</hotfix>" {
            if let Some((name, kind, codes)) = hotfix.take()
                && !codes.is_empty() {
                    stack.last_mut().unwrap().1.push(Node::Hotfix { name, kind, codes });
                }
        } else if line.starts_with("<code") {
            if let Some((true, code)) = parse_code(line) {
                match &mut hotfix {
                    Some((_, _, codes)) => codes.push(code),
                    None => stack.last_mut().unwrap().1.push(Node::Code(code)),
                }
            }
        } else if let Some(comment) = line.strip_prefix("<comment>") {
            let comment = comment.strip_suffix("</comment>").unwrap_or(comment);
            stack.last_mut().unwrap().1.push(Node::Comment(comment.to_string()));
        }
    }
    while stack.len() > 1 {
        let (name, children) = stack.pop().unwrap();
        stack.last_mut().unwrap().1.push(Node::Category { name, children });
    }
    stack.pop().unwrap().1
}

/// Keeps categories whose `/`-joined path starts with one of `include`
/// (everything if empty) and drops any whose path is listed in `exclude`.
pub fn filter(nodes: &[Node], include: &[&str], exclude: &[&str]) -> Vec<Node> {
    fn walk(nodes: &[Node], path: &str, include: &[&str], exclude: &[&str]) -> Vec<Node> {
        let mut out = Vec::new();
        for node in nodes {
            match node {
                Node::Category { name, children } => {
                    let p = if path.is_empty() { name.clone() } else { format!("{path}/{name}") };
                    if exclude.iter().any(|e| *e == p) {
                        continue;
                    }
                    let kids = walk(children, &p, include, exclude);
                    if kids.iter().any(|k| !matches!(k, Node::Comment(_))) {
                        out.push(Node::Category { name: name.clone(), children: kids });
                    }
                }
                other => {
                    let wanted = include.is_empty()
                        || include.iter().any(|i| path == *i || path.starts_with(&format!("{i}/")));
                    if wanted {
                        out.push(other.clone());
                    }
                }
            }
        }
        out
    }
    walk(nodes, "", include, exclude)
}

/// Every category path, for building include/exclude lists.
#[cfg_attr(not(test), allow(dead_code))]
pub fn paths(nodes: &[Node]) -> Vec<String> {
    fn walk(nodes: &[Node], path: &str, out: &mut Vec<String>) {
        for node in nodes {
            if let Node::Category { name, children } = node {
                let p = if path.is_empty() { name.clone() } else { format!("{path}/{name}") };
                out.push(p.clone());
                walk(children, &p, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(nodes, "", &mut out);
    out
}

/// Splits `set Obj Attr Value` / `set_cmp Obj Attr Old New` into
/// (object, attribute, old, new).
fn split_set(code: &str) -> Option<(String, String, String, String)> {
    let code = code.trim();
    let (cmd, rest) = code.split_once(char::is_whitespace)?;
    let rest = rest.trim_start();
    let (obj, rest) = rest.split_once(char::is_whitespace)?;
    let rest = rest.trim_start();
    let (attr_name, value) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let value = value.trim();
    if cmd.eq_ignore_ascii_case("set_cmp") {
        let (old, new) = value.split_once(char::is_whitespace)?;
        Some((obj.into(), attr_name.into(), old.into(), new.trim().into()))
    } else if cmd.eq_ignore_ascii_case("set") {
        Some((obj.into(), attr_name.into(), String::new(), value.into()))
    } else {
        None
    }
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_attr(value: &str) -> String {
    value.replace('"', "\\\"")
}

/// One input to a merge: a parsed, filtered tree and a display name.
pub struct Source {
    pub title: String,
    pub credit: String,
    pub nodes: Vec<Node>,
}

pub struct MergeInfo<'a> {
    pub game: &'a str,
    pub title: &'a str,
    pub author: &'a str,
    pub version: &'a str,
    pub description: &'a str,
}

/// Builds one offline BLCMM file from `gearbox` (the official hotfixes,
/// always first) and `sources`, in priority order (later wins).
pub fn merge(info: &MergeInfo, gearbox: &[Node], sources: &[Source]) -> String {
    const NL: &str = "\r\n";
    let mut out = String::new();
    let mut commands = Vec::new();
    let mut hotfixes: Vec<(HotfixKind, String)> = Vec::new();

    fn collect(nodes: &[Node], commands: &mut Vec<String>, hotfixes: &mut Vec<(HotfixKind, String)>) {
        for node in nodes {
            match node {
                Node::Category { children, .. } => collect(children, commands, hotfixes),
                Node::Code(code) => commands.push(code.clone()),
                Node::Hotfix { kind, codes, .. } => {
                    for code in codes {
                        hotfixes.push((kind.clone(), code.clone()));
                    }
                }
                Node::Comment(_) => {}
            }
        }
    }

    fn write_tree(out: &mut String, nodes: &[Node], depth: usize) {
        let pad = "\t".repeat(depth);
        for node in nodes {
            match node {
                Node::Category { name, children } => {
                    let _ = write!(out, "{pad}<category name=\"{}\">\r\n", escape_attr(name));
                    write_tree(out, children, depth + 1);
                    let _ = write!(out, "{pad}</category>\r\n");
                }
                Node::Comment(c) => {
                    let _ = write!(out, "{pad}<comment>{c}</comment>\r\n");
                }
                Node::Code(code) => {
                    let _ = write!(out, "{pad}<code profiles=\"default\">{code}</code>\r\n");
                }
                Node::Hotfix { name, kind, codes } => {
                    let target = match kind {
                        HotfixKind::Patch => String::new(),
                        HotfixKind::Level(l) => format!(" level=\"{}\"", if l.is_empty() { "None" } else { l }),
                        HotfixKind::OnDemand(p) => format!(" package=\"{p}\""),
                    };
                    let _ = write!(out, "{pad}<hotfix name=\"{}\"{target}>\r\n", escape_attr(name));
                    for code in codes {
                        let _ = write!(out, "{pad}\t<code profiles=\"default\">{code}</code>\r\n");
                    }
                    let _ = write!(out, "{pad}</hotfix>\r\n");
                }
            }
        }
    }

    // ---- tree
    out.push_str("<BLCMM v=\"1\">\r\n");
    out.push_str("#<!!!You opened a file saved with BLCMM in FilterTool. Please update to BLCMM to properly open this file!!!>\r\n");
    let _ = write!(out, "\t<head>{NL}\t\t<type name=\"{}\" offline=\"true\"/>{NL}", info.game);
    let _ = write!(out, "\t\t<profiles>{NL}\t\t\t<profile name=\"default\" current=\"true\"/>{NL}\t\t</profiles>{NL}\t</head>{NL}\t<body>{NL}");
    let _ = write!(out, "\t\t<category name=\"{}\">{NL}", escape_attr(info.title));
    for tag in [
        format!("@title {}", info.title),
        format!("@author {}", info.author),
        format!("@version {}", info.version),
        format!("@description {}", info.description),
        "Built on this PC by Vaulter from the original mods listed below; all credit to their authors.".into(),
    ] {
        let _ = write!(out, "\t\t\t<comment>{tag}</comment>{NL}");
    }
    let mut root = vec![Node::Category {
        name: "Original Gearbox Hotfix Data".into(),
        children: gearbox.to_vec(),
    }];
    for s in sources {
        let mut children = vec![Node::Comment(s.credit.clone())];
        children.extend(s.nodes.iter().cloned());
        root.push(Node::Category { name: s.title.clone(), children });
    }
    write_tree(&mut out, &root, 3);
    let _ = write!(out, "\t\t</category>{NL}\t</body>{NL}</BLCMM>{NL}{NL}");

    // ---- functional sections
    collect(gearbox, &mut commands, &mut hotfixes);
    let official: std::collections::HashSet<String> = hotfixes.iter().map(|(k, c)| hotfix_value(k, c)).collect();
    for s in sources {
        let mut theirs = Vec::new();
        collect(&s.nodes, &mut commands, &mut theirs);
        // Mods often embed copies of Gearbox's fixes; keep only one.
        hotfixes.extend(theirs.into_iter().filter(|(k, c)| !official.contains(&hotfix_value(k, c))));
    }

    out.push_str("#Commands:\r\n");
    for c in &commands {
        let _ = write!(out, "{c}{NL}");
    }
    out.push_str("\r\n#Hotfixes:\r\n");
    let service = "Transient.SparkServiceConfiguration_0";
    let _ = write!(out, "set {service} ServiceName Micropatch{NL}{NL}");
    let _ = write!(out, "set {service} ConfigurationGroup Default{NL}{NL}");
    let valid: Vec<(String, String)> = hotfixes
        .iter()
        .filter(|(_, c)| split_set(c).is_some())
        .enumerate()
        .map(|(i, (k, c))| (format!("{}-BLCMM{}", k.key_prefix(), i + 1), hotfix_value(k, c)))
        .collect();
    let keys: Vec<String> = valid.iter().map(|(k, _)| format!("\"{k}\"")).collect();
    let values: Vec<String> = valid.iter().map(|(_, v)| format!("\"{}\"", escape(v))).collect();
    let _ = write!(out, "set {service} Keys ({}){NL}", keys.join(","));
    let _ = write!(out, "set {service} Values ({}){NL}{NL}", values.join(","));
    let _ = write!(out, "set Transient.GearboxAccountData_1 Services ({service}){NL}");
    out
}

/// The raw Spark value for one hotfix code, e.g. `level,obj,attr,old,new`.
fn hotfix_value(kind: &HotfixKind, code: &str) -> String {
    let Some((obj, attr_name, old, new)) = split_set(code) else {
        return String::new();
    };
    match kind {
        HotfixKind::Patch => format!("{obj},{attr_name},{old},{new}"),
        HotfixKind::Level(l) | HotfixKind::OnDemand(l) => format!("{l},{obj},{attr_name},{old},{new}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "<BLCMM v=\"1\">\r\n\t<head>\r\n\t\t<type name=\"BL2\" offline=\"false\"/>\r\n\t</head>\r\n\t<body>\r\n\t\t<category name=\"Mod\">\r\n\t\t\t<comment>hi</comment>\r\n\t\t\t<category name=\"Fixes\">\r\n\t\t\t\t<code profiles=\"default\">set A.B C 1</code>\r\n\t\t\t\t<code profiles=\"\">set A.B D 2</code>\r\n\t\t\t\t<hotfix name=\"X\" level=\"Map_P\">\r\n\t\t\t\t\t<code profiles=\"default\">set_cmp O.P Q.R 1 2</code>\r\n\t\t\t\t</hotfix>\r\n\t\t\t</category>\r\n\t\t\t<category name=\"Balance \\\"stuff\\\"\">\r\n\t\t\t\t<hotfix name=\"Y\" package=\"GD_Pkg\">\r\n\t\t\t\t\t<code>set O.P Text \"quoted, value\"</code>\r\n\t\t\t\t</hotfix>\r\n\t\t\t</category>\r\n\t\t</category>\r\n\t</body>\r\n</BLCMM>\r\n\r\n#Commands:\r\nset A.B C 1\r\n";

    #[test]
    fn parses_tree_profiles_and_hotfixes() {
        let nodes = parse(SAMPLE);
        assert_eq!(paths(&nodes), vec!["Mod", "Mod/Fixes", "Mod/Balance \"stuff\""]);
        let fixes = filter(&nodes, &["Mod/Fixes"], &[]);
        let mut cmds = Vec::new();
        let mut hfs = Vec::new();
        fn walk(n: &[Node], c: &mut Vec<String>, h: &mut Vec<String>) {
            for x in n {
                match x {
                    Node::Category { children, .. } => walk(children, c, h),
                    Node::Code(s) => c.push(s.clone()),
                    Node::Hotfix { codes, .. } => h.extend(codes.clone()),
                    _ => {}
                }
            }
        }
        walk(&fixes, &mut cmds, &mut hfs);
        assert_eq!(cmds, vec!["set A.B C 1"], "disabled codes are dropped");
        assert_eq!(hfs, vec!["set_cmp O.P Q.R 1 2"]);
        assert!(filter(&nodes, &[], &["Mod/Balance \"stuff\""]).len() == 1);
    }

    #[test]
    fn merged_file_has_offline_hotfix_service() {
        let nodes = parse(SAMPLE);
        let gearbox = vec![Node::Hotfix {
            name: "GBX".into(),
            kind: HotfixKind::Level(String::new()),
            codes: vec!["set_cmp G.H I 0.5 1".into()],
        }];
        let text = merge(
            &MergeInfo { game: "BL2", title: "T", author: "A", version: "1", description: "D" },
            &gearbox,
            &[Source { title: "Mod".into(), credit: "by someone".into(), nodes }],
        );
        assert!(text.contains("offline=\"true\""));
        assert!(text.contains("#Commands:\r\nset A.B C 1\r\n"));
        assert!(text.contains(
            "Keys (\"SparkLevelPatchEntry-BLCMM1\",\"SparkLevelPatchEntry-BLCMM2\",\"SparkOnDemandPatchEntry-BLCMM3\")"
        ));
        assert!(text.contains("\",G.H,I,0.5,1\",\"Map_P,O.P,Q.R,1,2\",\"GD_Pkg,O.P,Text,,\\\"quoted, value\\\"\""));
        assert!(text.ends_with("set Transient.GearboxAccountData_1 Services (Transient.SparkServiceConfiguration_0)\r\n"));
        // Re-parsing our own output keeps the tree intact.
        let again = parse(&text);
        assert!(paths(&again).iter().any(|p| p.ends_with("Mod/Fixes")));
    }

    #[test]
    fn flat_files_become_code_lists() {
        let nodes = parse("set A B 1\r\nsay hi\r\nset Transient.SparkServiceConfiguration_6 Keys ()\r\n");
        assert_eq!(nodes, vec![Node::Code("set A B 1".into())]);
    }
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    #[test]
    fn escaping_and_ansi_survive_the_merge() {
        let src = "<BLCMM v=\"1\">\r\n\t<body>\r\n\t\t<category name=\"M\">\r\n\t\t\t<hotfix name=\"H\">\r\n\t\t\t\t<code profiles=\"default\">set O.P Text \"Caf\u{e9}, a \\\\ b\"</code>\r\n\t\t\t</hotfix>\r\n\t\t</category>\r\n\t</body>\r\n</BLCMM>\r\n";
        let nodes = parse(src);
        let text = merge(
            &MergeInfo { game: "BL2", title: "T", author: "A", version: "1", description: "D" },
            &[],
            &[Source { title: "M".into(), credit: "c".into(), nodes }],
        );
        // One key, one value, with quotes and backslashes escaped.
        assert!(text.contains("Keys (\"SparkPatchEntry-BLCMM1\")"));
        assert!(text.contains("Values (\"O.P,Text,,\\\"Caf\u{e9}, a \\\\\\\\ b\\\"\")"));
        // Latin-1 round trip of the written bytes.
        let bytes = encode(&text);
        assert!(bytes.contains(&0xE9));
        assert_eq!(decode(&bytes), text);
    }
}
