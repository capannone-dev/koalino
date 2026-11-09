//! Specifies sealed attachment files (#99): what is written is not the clear contents, reads give
//! the contents back whole, in parts from an offset, or as a clear copy; a cut, altered or
//! foreign file is refused; clear files from before #99 are still read and are sealed in place
//! once. Each test uses its own temporary directory.

use std::{
    io::Read,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{
    adapters::storage::sealed_files::{SealedFiles, is_sealed},
    application::crypto::voice_encryptor::ephemeral_cipher,
};

/// A temporary directory deleted when dropped.
struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("sealed-files-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sealed_files() -> SealedFiles {
    SealedFiles::new(ephemeral_cipher())
}

/// Contents spanning several records, with a recognisable pattern.
fn contents(length: usize) -> Vec<u8> {
    (0..length).map(|index| (index % 251) as u8).collect()
}

fn write(files: &SealedFiles, path: &Path, data: &[u8]) -> u64 {
    files.write_new(path, &mut &data[..]).unwrap()
}

#[test]
fn written_file_is_sealed_and_reads_back_whole() {
    let directory = TemporaryDirectory::new();
    let files = sealed_files();
    let path = directory.join("photo.jpg");
    let data = contents(600 * 1024 + 17);

    assert_eq!(write(&files, &path, &data), data.len() as u64);

    let on_disk = std::fs::read(&path).unwrap();
    assert!(is_sealed(&path).unwrap());
    assert!(
        !on_disk
            .windows(64)
            .any(|window| window == &data[1000..1064]),
        "the clear contents must not be on disk"
    );
    assert_eq!(files.read_all(&path).unwrap().as_slice(), data.as_slice());
    assert_eq!(files.clear_len(&path).unwrap(), data.len() as u64);
}

#[test]
fn empty_file_round_trips() {
    let directory = TemporaryDirectory::new();
    let files = sealed_files();
    let path = directory.join("empty.txt");

    assert_eq!(write(&files, &path, b""), 0);
    assert!(files.read_all(&path).unwrap().is_empty());
    assert_eq!(files.clear_len(&path).unwrap(), 0);
}

#[test]
fn write_new_never_overwrites() {
    let directory = TemporaryDirectory::new();
    let files = sealed_files();
    let path = directory.join("note.md");
    write(&files, &path, b"first");

    assert!(files.write_new(&path, &mut &b"second"[..]).is_err());
    assert_eq!(files.read_all(&path).unwrap().as_slice(), b"first");
}

#[cfg(unix)]
#[test]
fn written_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let directory = TemporaryDirectory::new();
    let files = sealed_files();
    let path = directory.join("note.md");
    write(&files, &path, b"text");

    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn reader_starts_at_any_offset() {
    let directory = TemporaryDirectory::new();
    let files = sealed_files();
    let path = directory.join("slides.pdf");
    let data = contents(700 * 1024);
    write(&files, &path, &data);

    for offset in [0, 1, 256 * 1024 - 1, 256 * 1024, 300_000, data.len() as u64] {
        let mut reader = files.open(&path).unwrap();
        assert_eq!(reader.clear_len().unwrap(), data.len() as u64);
        reader.seek_to(offset).unwrap();
        let mut part = Vec::new();
        reader.by_ref().take(70_000).read_to_end(&mut part).unwrap();
        let start = offset as usize;
        let end = (start + 70_000).min(data.len());
        assert_eq!(part, &data[start..end], "offset {offset}");
    }
}

#[test]
fn file_sealed_with_another_key_is_refused() {
    let directory = TemporaryDirectory::new();
    let path = directory.join("photo.png");
    write(&sealed_files(), &path, b"private picture");

    assert!(sealed_files().read_all(&path).is_err());
}

#[test]
fn cut_file_is_refused() {
    let directory = TemporaryDirectory::new();
    let files = sealed_files();
    let path = directory.join("doc.pdf");
    write(&files, &path, &contents(600 * 1024));
    let on_disk = std::fs::read(&path).unwrap();

    // Cut at a record boundary: everything left authenticates, but the end record is missing.
    for keep in [on_disk.len() - 1, on_disk.len() / 2, 24] {
        std::fs::write(&path, &on_disk[..keep]).unwrap();
        assert!(files.read_all(&path).is_err(), "kept {keep} bytes");
    }
}

#[test]
fn altered_file_is_refused() {
    let directory = TemporaryDirectory::new();
    let files = sealed_files();
    let path = directory.join("doc.pdf");
    write(&files, &path, &contents(10_000));
    let mut on_disk = std::fs::read(&path).unwrap();
    let middle = on_disk.len() / 2;
    on_disk[middle] ^= 0x40;
    std::fs::write(&path, &on_disk).unwrap();

    assert!(files.read_all(&path).is_err());
}

#[test]
fn records_moved_between_files_are_refused() {
    let directory = TemporaryDirectory::new();
    let files = sealed_files();
    let first = directory.join("a.txt");
    let second = directory.join("b.txt");
    write(&files, &first, b"alpha");
    write(&files, &second, b"bravo");
    let first_bytes = std::fs::read(&first).unwrap();
    let mut second_bytes = std::fs::read(&second).unwrap();
    // Keep the second file's header, take the first file's records.
    second_bytes.truncate(24);
    second_bytes.extend_from_slice(&first_bytes[24..]);
    std::fs::write(&second, &second_bytes).unwrap();

    assert!(files.read_all(&second).is_err());
}

#[test]
fn clear_file_from_before_sealing_is_read_and_sealed_once() {
    let directory = TemporaryDirectory::new();
    let files = sealed_files();
    let path = directory.join("old.txt");
    std::fs::write(&path, b"written before #99").unwrap();

    assert!(!is_sealed(&path).unwrap());
    assert_eq!(
        files.read_all(&path).unwrap().as_slice(),
        b"written before #99"
    );
    assert_eq!(files.clear_len(&path).unwrap(), 18);

    assert!(files.seal_in_place(&path).unwrap());
    assert!(is_sealed(&path).unwrap());
    assert!(!files.seal_in_place(&path).unwrap(), "already sealed");
    assert_eq!(
        files.read_all(&path).unwrap().as_slice(),
        b"written before #99"
    );
    let leftovers: Vec<_> = std::fs::read_dir(&directory.0).unwrap().flatten().collect();
    assert_eq!(leftovers.len(), 1, "no temporary file is left behind");
}

#[test]
fn appender_builds_a_file_from_parts() {
    let directory = TemporaryDirectory::new();
    let files = sealed_files();
    let path = directory.join("received.part");
    let data = contents(200_000);

    let mut appender = files.create_appender(&path).unwrap();
    for part in data.chunks(64 * 1024) {
        appender.append(part).unwrap();
    }
    // Before it is finished the file has no end and is refused.
    assert!(files.read_all(&path).is_err());
    assert_eq!(appender.finish().unwrap(), data.len() as u64);

    assert_eq!(files.read_all(&path).unwrap().as_slice(), data.as_slice());
}

#[test]
fn clear_copy_writes_the_contents() {
    let directory = TemporaryDirectory::new();
    let files = sealed_files();
    let path = directory.join("photo.jpg");
    let data = contents(300_000);
    write(&files, &path, &data);
    let copy = directory.join("copy.jpg");
    std::fs::write(&copy, b"older contents that are longer than nothing").unwrap();

    assert_eq!(files.copy_clear(&path, &copy).unwrap(), data.len() as u64);
    assert_eq!(std::fs::read(&copy).unwrap(), data);
}
