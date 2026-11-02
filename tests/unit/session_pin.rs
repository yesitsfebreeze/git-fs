use super::resolve_from;
use std::path::Path;

fn pin(dir: &Path, pid: u32, session: &str) {
	std::fs::write(dir.join(format!("{pid}.session")), format!("{session}\n")).unwrap();
}

#[test]
fn the_nearest_pinned_ancestor_wins_and_unknown_ids_are_skipped() {
	let dir = tempfile::tempdir().unwrap();
	pin(dir.path(), 7, "host");
	pin(dir.path(), 42, "nearest");
	assert_eq!(
		resolve_from(dir.path(), &[99, 42, 7]).as_deref(),
		Some("nearest")
	);
	assert_eq!(resolve_from(dir.path(), &[99]).as_deref(), None);
}

#[test]
fn an_invalid_or_stale_pin_is_ignored() {
	let dir = tempfile::tempdir().unwrap();
	pin(dir.path(), 1, "../escape");
	pin(dir.path(), 2, "fresh");
	let old = dir.path().join("2.session");
	let file = std::fs::OpenOptions::new().write(true).open(&old).unwrap();
	file.set_modified(
		std::time::SystemTime::now() - std::time::Duration::from_secs(8 * 24 * 60 * 60),
	)
	.unwrap();
	assert_eq!(resolve_from(dir.path(), &[1, 2]).as_deref(), None);
}
