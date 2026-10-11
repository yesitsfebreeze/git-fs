//! Secrets scan, a faithful port of autogit's scanning: patterns are
//! conservative to avoid false positives; template/placeholder files are meant
//! to be committed and are skipped entirely (filename AND content); placeholders
//! are tested against the matched token only, never the whole line.

use regex::Regex;
use std::sync::LazyLock;

pub struct Pattern {
	pub name: &'static str,
	re: Regex,
}

impl Pattern {
	fn new(name: &'static str, re: &str) -> Pattern {
		Pattern {
			name,
			re: Regex::new(re).expect("valid pattern"),
		}
	}
}

pub static PATTERNS: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
	vec![
		Pattern::new("AWS access key", r"AKIA[0-9A-Z]{16}"),
		Pattern::new(
			"Private key block",
			r"-----BEGIN (RSA |EC |OPENSSH |DSA |PGP )?PRIVATE KEY-----",
		),
		Pattern::new(
			"Generic API key assignment",
			r#"(?i)(api[_-]?key|secret[_-]?key|access[_-]?token|auth[_-]?token)["']?\s*[:=]\s*["'][A-Za-z0-9_\-]{20,}["']"#,
		),
		Pattern::new("OpenAI key", r"\bsk-[A-Za-z0-9_\-]{20,}"),
		Pattern::new("Anthropic key", r"sk-ant-[A-Za-z0-9_\-]{20,}"),
		Pattern::new("GitHub token", r"gh[pousr]_[A-Za-z0-9]{36,}"),
		Pattern::new("Slack token", r"xox[baprs]-[A-Za-z0-9\-]{10,}"),
		Pattern::new("Google API key", r"AIza[0-9A-Za-z_\-]{35}"),
		Pattern::new(
			"JWT",
			r"eyJ[A-Za-z0-9_\-]{10,}\.eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}",
		),
	]
});

const SENSITIVE_FILES: [&str; 5] = [
	r"^\.env(\..+)?$",
	r"\.pem$",
	r"\.key$",
	r"id_rsa",
	r"credentials\.json$",
];
const TEMPLATE_FILE: &str = r"\.(example|sample|template|dist)$";
const PLACEHOLDER: &str =
	r"your|example|sample|placeholder|changeme|dummy|redacted|xxxx|1234567890";

fn re(pat: &str) -> Regex {
	Regex::new(pat).expect("valid pattern")
}

fn is_template(base: &str) -> bool {
	re(TEMPLATE_FILE).is_match(base)
}

fn is_sensitive(base: &str) -> bool {
	SENSITIVE_FILES.iter().any(|p| re(p).is_match(base))
}

fn base_of(f: &str) -> String {
	std::path::Path::new(f)
		.file_name()
		.map(|b| b.to_string_lossy().into_owned())
		.unwrap_or_else(|| f.to_owned())
}

pub struct Finding {
	pub file: String,
	pub issue: String,
}

/// Scan a unified diff that has added lines (as `git diff --unified=0`
/// produces): files named like templates are skipped; sensitive filenames are
/// flagged; real secrets on added lines are flagged, placeholders are not.
pub fn scan_diff(diff: &str) -> Vec<Finding> {
	let mut findings = Vec::new();
	let mut current_file = String::new();
	let mut in_template = false;
	for line in diff.lines() {
		if let Some(rest) = line.strip_prefix("+++ b/") {
			current_file = rest.to_owned();
			in_template = is_template(&base_of(&current_file));
			if in_template {
				continue;
			}
			if is_sensitive(&base_of(&current_file)) {
				findings.push(Finding {
					file: current_file.clone(),
					issue: "sensitive filename".into(),
				});
			}
			continue;
		}
		if in_template || !line.starts_with('+') {
			continue;
		}
		for p in PATTERNS.iter() {
			if let Some(m) = p.re.find(line) {
				if !re(PLACEHOLDER).is_match(m.as_str()) {
					findings.push(Finding {
						file: current_file.clone(),
						issue: p.name.to_owned(),
					});
				}
			}
		}
	}
	findings
}

/// A secret anywhere in `text` (used for prompt-derived subjects, which must
/// never carry one).
pub fn has_secret(text: &str) -> bool {
	PATTERNS.iter().any(|p| p.re.is_match(text))
}

/// Standalone scan of whole files on disk, honoring the same filename and
/// placeholder rules. Used by the `scan` op before anything is staged.
pub fn scan_file(path: &str, name: &str, content: &str) -> Vec<Finding> {
	if is_template(name) {
		return vec![];
	}
	let mut findings = Vec::new();
	if is_sensitive(name) {
		findings.push(Finding {
			file: path.to_owned(),
			issue: "sensitive filename".into(),
		});
	}
	for (idx, line) in content.lines().enumerate() {
		for p in PATTERNS.iter() {
			if let Some(m) = p.re.find(line) {
				if !re(PLACEHOLDER).is_match(m.as_str()) {
					findings.push(Finding {
						file: format!("{path}:{}", idx + 1),
						issue: p.name.to_owned(),
					});
				}
			}
		}
	}
	findings
}

#[cfg(test)]
#[path = "../tests/unit/secrets/tests.rs"]
mod tests;
