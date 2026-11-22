//! Git LFS without the `git-lfs` binary (#1121).
//!
//! libgit2 does not run external filters, so a file that `.gitattributes`
//! marks `filter=lfs` would be committed as its raw bytes. Instead, the
//! content goes into `.git/lfs/objects/` and the repository records the
//! small pointer file `git-lfs` would have written. Which paths are LFS is
//! read from the repository's attributes, so every `filter=lfs` rule
//! applies, whoever wrote it.

use std::path::{Path, PathBuf};

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

/// The content hash an LFS pointer file names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LfsPointer {
    pub(crate) oid: String,
}

/// Read `content` as an LFS pointer. The hash becomes a file name, so only
/// 64 lowercase hex digits are accepted.
pub(crate) fn parse_pointer(content: &[u8]) -> Option<LfsPointer> {
    if !is_pointer(content) {
        return None;
    }
    let text = std::str::from_utf8(content).ok()?;
    let oid = text.lines().find_map(|l| l.strip_prefix("oid sha256:"))?;
    let is_hash = oid.len() == 64 && oid.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    is_hash.then(|| LfsPointer {
        oid: oid.to_owned(),
    })
}

/// The pointer blob `id` holds, if it is one. A zero id (no file on that
/// side) and a non-blob entry (a submodule) hold none.
pub(crate) fn pointer_in_blob(
    repo: &git2::Repository,
    id: git2::Oid,
) -> Result<Option<LfsPointer>, git2::Error> {
    if id.is_zero() {
        return Ok(None);
    }
    match repo.odb()?.read_header(id) {
        Ok((size, git2::ObjectType::Blob)) if size <= POINTER_MAX_LEN => {
            Ok(parse_pointer(repo.find_blob(id)?.content()))
        }
        _ => Ok(None),
    }
}

/// Where the object of `oid` lives: the common git folder, which every linked
/// worktree of the repository shares.
pub(crate) fn object_path(repo: &git2::Repository, oid: &str) -> PathBuf {
    repo.commondir()
        .join("lfs")
        .join("objects")
        .join(&oid[0..2])
        .join(&oid[2..4])
        .join(oid)
}

/// Whether the file at `object` exists and hashes to `oid`.
pub(crate) fn object_holds(object: &Path, oid: &str) -> bool {
    std::fs::read(object).is_ok_and(|bytes| pointer_for(&bytes).1 == oid)
}

/// Whether a working file's `bytes` are the content `pointer` names, or the
/// pointer itself (a checkout without git-lfs).
pub(crate) fn holds_content_of(bytes: &[u8], pointer: &LfsPointer) -> bool {
    parse_pointer(bytes).map_or_else(|| pointer_for(bytes).1 == pointer.oid, |p| p == *pointer)
}

/// Keep a working file's verified content in the object store before a
/// switch replaces it: after a prune it can be the only copy.
pub(crate) fn keep_content(
    repo: &git2::Repository,
    pointer: &LfsPointer,
    bytes: &[u8],
) -> std::io::Result<()> {
    let object = object_path(repo, &pointer.oid);
    if is_pointer(bytes) || object.exists() {
        return Ok(());
    }
    if let Some(dir) = object.parent() {
        std::fs::create_dir_all(dir)?;
    }
    signex_types::atomic_io::atomic_write(&object, bytes)
}

/// Store `content` under `.git/lfs/objects/` (once per content) and write
/// its pointer as a blob. A file that already is a pointer (a checkout
/// without git-lfs) is stored as it is.
fn lfs_blob(repo: &git2::Repository, content: &[u8]) -> Result<git2::Oid, git2::Error> {
    if is_pointer(content) {
        return repo.blob(content);
    }
    let (pointer, oid) = pointer_for(content);
    let object = object_path(repo, &oid);
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
    blob_for_bytes(repo, rel_path, &content)
}

/// The blob to commit for `content` as the file `rel_path`: its LFS pointer
/// when the attributes say `filter=lfs`, else the bytes themselves. Unlike
/// [`blob_for_path`] it never looks at the working tree, so the commit holds
/// exactly the bytes the caller wrote or read, whatever the file says now.
pub(crate) fn blob_for_bytes(
    repo: &git2::Repository,
    rel_path: &Path,
    content: &[u8],
) -> Result<git2::Oid, git2::Error> {
    if !is_lfs_path(repo, rel_path)? {
        return repo.blob(content);
    }
    let blob = lfs_blob(repo, content)?;
    // The object now lives only in `.git/lfs/objects/`: make sure a push
    // uploads it, even if LFS was enabled before any model existed.
    install_pre_push_hook(repo)?;
    Ok(blob)
}

/// The blob id the working-tree file at `rel_path` would be committed as:
/// the hash of its bytes, or of its LFS pointer for a `filter=lfs` path.
/// Nothing is written to the object database.
pub(crate) fn working_blob_id(
    repo: &git2::Repository,
    workdir: &Path,
    rel_path: &Path,
) -> Result<git2::Oid, git2::Error> {
    let abs = workdir.join(rel_path);
    if !is_lfs_path(repo, rel_path)? {
        return git2::Oid::hash_file(git2::ObjectType::Blob, abs);
    }
    let content = std::fs::read(&abs).map_err(io_error)?;
    if is_pointer(&content) {
        return git2::Oid::hash_object(git2::ObjectType::Blob, &content);
    }
    git2::Oid::hash_object(git2::ObjectType::Blob, &pointer_for(&content).0)
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
    signex_types::atomic_io::atomic_write(&hook, PRE_PUSH_HOOK.as_bytes()).map_err(io_error)?;
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
