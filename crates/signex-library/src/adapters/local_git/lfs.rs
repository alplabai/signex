//! Git LFS without the `git-lfs` binary (#1121).
//!
//! libgit2 does not run external filters, so a file that `.gitattributes`
//! marks `filter=lfs` would be committed as its raw bytes. Instead, the
//! content goes into `.git/lfs/objects/` and the repository records the
//! small pointer file `git-lfs` would have written. Which paths are LFS is
//! read from the repository's attributes, so every `filter=lfs` rule
//! applies, whoever wrote it.

use std::io::Write;
use std::path::Path;

use sha2::{Digest, Sha256};

/// First line of every LFS pointer file (the spec URL is part of the
/// format, not a link to fetch).
const POINTER_VERSION: &[u8] = b"version https://git-lfs.github.com/spec/v1\n";

/// Pointer files are tiny; anything this large that starts like one is
/// content, not a pointer.
const POINTER_MAX_LEN: usize = 1024;

/// The `pre-push` hook a fresh repository gets, so a plain `git push`
/// uploads the stored objects (git-lfs installs the same kind of hook).
const PRE_PUSH_HOOK: &str = "#!/bin/sh\n\
# Written by Signex: this repository stores 3D models with Git LFS.\n\
command -v git-lfs >/dev/null 2>&1 || {\n\
  echo >&2 \"This repository stores files with Git LFS, but git-lfs is not installed.\"\n\
  echo >&2 \"Install it (https://git-lfs.com), run 'git lfs install', then push again.\"\n\
  exit 2\n\
}\n\
git lfs pre-push \"$@\"\n";

/// Whether the repository's attributes route `rel_path` through LFS.
pub(crate) fn is_lfs_path(repo: &git2::Repository, rel_path: &Path) -> Result<bool, git2::Error> {
    Ok(repo.get_attr(rel_path, "filter", git2::AttrCheckFlags::FILE_THEN_INDEX)? == Some("lfs"))
}

/// The LFS pointer for `content`: `(pointer bytes, sha256 hex)`.
pub(crate) fn pointer_for(content: &[u8]) -> (Vec<u8>, String) {
    let oid = format!("{:x}", Sha256::digest(content));
    let mut pointer = POINTER_VERSION.to_vec();
    pointer.extend_from_slice(format!("oid sha256:{oid}\nsize {}\n", content.len()).as_bytes());
    (pointer, oid)
}

fn is_pointer(content: &[u8]) -> bool {
    content.len() <= POINTER_MAX_LEN && content.starts_with(POINTER_VERSION)
}

/// Store `content` under `.git/lfs/objects/` (once per content) and write
/// its pointer as a blob. A file that already is a pointer (a checkout
/// without git-lfs) is stored as it is.
fn lfs_blob(repo: &git2::Repository, content: &[u8]) -> Result<git2::Oid, git2::Error> {
    if is_pointer(content) {
        return repo.blob(content);
    }
    let (pointer, oid) = pointer_for(content);
    let object = repo
        .path()
        .join("lfs")
        .join("objects")
        .join(&oid[0..2])
        .join(&oid[2..4])
        .join(&oid);
    if !object.exists() {
        let dir = object.parent().expect("object path has a parent");
        std::fs::create_dir_all(dir).map_err(io_error)?;
        signex_types::atomic_io::atomic_write(&object, content).map_err(io_error)?;
    }
    repo.blob(&pointer)
}

/// The blob to commit for the working-tree file at `rel_path`: its LFS
/// pointer when the attributes say `filter=lfs`, else its bytes.
pub(crate) fn blob_for_path(
    repo: &git2::Repository,
    workdir: &Path,
    rel_path: &Path,
) -> Result<git2::Oid, git2::Error> {
    if !is_lfs_path(repo, rel_path)? {
        return repo.blob_path(&workdir.join(rel_path));
    }
    let content = std::fs::read(workdir.join(rel_path)).map_err(io_error)?;
    lfs_blob(repo, &content)
}

/// Replace every staged `filter=lfs` entry in `index` with its pointer.
/// Returns how many entries were converted.
pub(crate) fn convert_index(
    repo: &git2::Repository,
    index: &mut git2::Index,
) -> Result<usize, git2::Error> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| git2::Error::from_str("repository has no working tree"))?
        .to_path_buf();
    let mut converted = Vec::new();
    for entry in index.iter() {
        let rel = String::from_utf8_lossy(&entry.path).into_owned();
        if is_lfs_path(repo, Path::new(&rel))? {
            converted.push(entry);
        }
    }
    let count = converted.len();
    for mut entry in converted {
        let rel = String::from_utf8_lossy(&entry.path).into_owned();
        let content = std::fs::read(workdir.join(&rel)).map_err(io_error)?;
        // The entry keeps the working file's stat data, as `git add` with
        // git-lfs does; only the recorded content becomes the pointer.
        entry.id = lfs_blob(repo, &content)?;
        index.add(&entry)?;
    }
    Ok(count)
}

/// Give a repository that has LFS paths the `pre-push` hook that uploads
/// their objects. Leaves an existing hook alone.
pub(crate) fn install_pre_push_hook(repo: &git2::Repository) -> Result<(), git2::Error> {
    let hooks = repo.path().join("hooks");
    let hook = hooks.join("pre-push");
    if hook.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(&hooks).map_err(io_error)?;
    let mut file = std::fs::File::create(&hook).map_err(io_error)?;
    file.write_all(PRE_PUSH_HOOK.as_bytes()).map_err(io_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))
            .map_err(io_error)?;
    }
    Ok(())
}

fn io_error(e: std::io::Error) -> git2::Error {
    git2::Error::from_str(&format!("git lfs: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pointer_names_the_content_hash_and_size() {
        let (pointer, oid) = pointer_for(b"hello\n");
        assert_eq!(
            oid,
            "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03"
        );
        assert_eq!(
            String::from_utf8(pointer).unwrap(),
            "version https://git-lfs.github.com/spec/v1\n\
             oid sha256:5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03\n\
             size 6\n"
        );
    }

    #[test]
    fn a_pointer_is_not_wrapped_again() {
        let (pointer, _) = pointer_for(b"model");
        assert!(is_pointer(&pointer));
        assert!(!is_pointer(b"ISO-10303-21;"));
    }
}
