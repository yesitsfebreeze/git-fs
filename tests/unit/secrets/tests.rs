use super::*;

#[test]
fn real_key_on_added_line_finds() {
	let diff = "+++ b/conf.json\n- old\n+{\"aws\": \"AKIAIOSFODNN7EXAMPLE\"}\n";
	let hits = scan_diff(diff);
	assert_eq!(hits.len(), 1);
	assert_eq!(hits[0].file, "conf.json");
	assert_eq!(hits[0].issue, "AWS access key");
}

#[test]
fn placeholder_token_is_ignored() {
	let diff = "+++ b/env.ts\n+const key = \"sk-your-very-long-api-key-0123456789\"\n";
	assert!(scan_diff(diff).is_empty());
}

#[test]
fn context_lines_are_not_scanned() {
	let diff = "+++ b/x\n@@ -1 +1 @@\n AKIAIOSFODNN7EXAMPLE\n+fine\n";
	assert!(scan_diff(diff).is_empty());
}

#[test]
fn template_files_are_skipped_entirely() {
	let diff =
		"+++ b/.env.example\n+AKIAIOSFODNN7EXAMPLE\n+sk-ant-api123456789012345678901234567890\n";
	assert!(scan_diff(diff).is_empty());
}

#[test]
fn sensitive_filename_is_flagged_even_with_plain_content() {
	let diff = "+++ b/.env\n+X=1\n";
	let hits = scan_diff(diff);
	assert_eq!(hits.len(), 1);
	assert_eq!(hits[0].issue, "sensitive filename");
}

#[test]
fn scan_file_matches() {
	// Template: content ignored entirely even with a real key.
	assert!(scan_file("a", ".env.example", "KEY=AKIAIOSFODNN7EXAMPLE").is_empty());
	// Sensitive filename flagged, no key needed.
	let hits = scan_file(".env", ".env", "X=1");
	assert_eq!(hits.len(), 1);
	assert_eq!(hits[0].issue, "sensitive filename");
	// Real key in a normal file flagged at its line.
	let hits = scan_file("t.ts", "t.ts", "x\nkey=AKIAIOSFODNN7EXAMPLE\n");
	assert_eq!(hits.len(), 1);
	assert!(hits[0].file.ends_with(":2"));
}

#[test]
fn has_secret_detects_and_allows_plain() {
	assert!(has_secret(
		"rotate sk-ant-api123456789012345678901234567890 now"
	));
	assert!(has_secret("AKIAIOSFODNN7EXAMPLE"));
	assert!(!has_secret("no secrets here"));
}
