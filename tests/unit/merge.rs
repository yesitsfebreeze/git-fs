use crate::merge::merge3;

#[test]
fn separate_line_changes_merge_cleanly() {
	let base = b"title\n\nrow one\nrow two\n";
	let ours = b"title\n\nrow one\nrow two\nrow three\n";
	let theirs = b"title\n\nrow zero\nrow one\nrow two\n";
	let merged = merge3(base, ours, theirs).unwrap().unwrap();
	assert_eq!(
		String::from_utf8(merged).unwrap(),
		"title\n\nrow zero\nrow one\nrow two\nrow three\n"
	);
}

#[test]
fn two_appends_to_one_table_both_land() {
	let base = b"| id | state |\n| 1 | done |\n";
	let ours = b"| id | state |\n| 1 | done |\n| 2 | ours |\n";
	let theirs = b"| id | state |\n| 1 | done |\n| 3 | theirs |\n";
	let merged = merge3(base, ours, theirs).unwrap().unwrap();
	assert_eq!(
		String::from_utf8(merged).unwrap(),
		"| id | state |\n| 1 | done |\n| 2 | ours |\n| 3 | theirs |\n"
	);
}

#[test]
fn an_overlapping_change_is_not_guessed() {
	let base = b"one\ntwo\nthree\n";
	let ours = b"one\nours\nthree\n";
	let theirs = b"one\ntheirs\nthree\n";
	assert_eq!(merge3(base, ours, theirs).unwrap(), None);
}

#[test]
fn one_unchanged_side_takes_the_other_verbatim() {
	let base = b"same\n";
	let ours = b"same\n";
	let theirs = b"changed\n";
	assert_eq!(
		merge3(base, ours, theirs).unwrap().unwrap(),
		b"changed\n".to_vec()
	);
	let binary = [0xff, 0x00, 0x01];
	assert_eq!(merge3(base, binary.as_slice(), theirs).unwrap(), None);
}
