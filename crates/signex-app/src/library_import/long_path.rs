//! The extended-length (`\\?\`) form of a Windows path, for the calls the
//! import makes to the system itself (#3234). std adds the prefix for its
//! own calls; `CreateFileW` and `MoveFileExW` given a plain path stop at 260
//! characters unless long paths are enabled machine-wide, which Windows
//! leaves off by default.
//!
//! The conversion works on UTF-16 units and does no IO, so its tests run on
//! every system; only [`wide`], which makes the path absolute first, is
//! Windows-only.

const BACKSLASH: u16 = b'\\' as u16;

fn is_separator(unit: u16) -> bool {
    unit == BACKSLASH || unit == u16::from(b'/')
}

fn units(text: &str) -> impl Iterator<Item = u16> + '_ {
    text.encode_utf16()
}

/// The extended-length form of the absolute path `absolute`: `\\?\C:\…`
/// for a drive path, `\\?\UNC\server\share\…` for a share. Separators
/// become `\`, empty and `.` components go and `..` takes the one before
/// it (never the drive root or the share), because the system does none of
/// that for an extended-length path. A path already in `\\?\` or `\\.\`
/// form is returned as it is; `None` for a path that is not absolute.
pub(super) fn extended_length(absolute: &[u16]) -> Option<Vec<u16>> {
    let starts_with_two_separators =
        absolute.len() >= 2 && is_separator(absolute[0]) && is_separator(absolute[1]);
    if starts_with_two_separators
        && absolute.len() >= 4
        && matches!(absolute[2], 0x3F | 0x2E) // `?` or `.`
        && is_separator(absolute[3])
    {
        return Some(absolute.to_vec());
    }
    let is_drive = absolute.len() >= 3
        && u8::try_from(absolute[0]).is_ok_and(|c| c.is_ascii_alphabetic())
        && absolute[1] == u16::from(b':')
        && is_separator(absolute[2]);
    let (mut out, rest, kept) = if is_drive {
        let mut out: Vec<u16> = units(r"\\?\").collect();
        out.extend_from_slice(&absolute[..2]);
        (out, &absolute[3..], 0)
    } else if starts_with_two_separators {
        // The server and the share are the root `..` never climbs past.
        (units(r"\\?\UNC").collect(), &absolute[2..], 2)
    } else {
        return None;
    };
    let mut parts: Vec<&[u16]> = Vec::new();
    for part in rest.split(|&unit| is_separator(unit)) {
        match part {
            [] | [0x2E] => {}
            [0x2E, 0x2E] => {
                if parts.len() > kept {
                    parts.pop();
                }
            }
            _ => parts.push(part),
        }
    }
    if parts.len() < kept {
        return None;
    }
    if parts.is_empty() {
        out.push(BACKSLASH);
    }
    for part in parts {
        out.push(BACKSLASH);
        out.extend_from_slice(part);
    }
    Some(out)
}

/// `path` as a NUL-terminated wide string for a system call: absolute, in
/// extended-length form, so its length is not limited to 260 characters.
#[cfg(windows)]
pub(super) fn wide(path: &std::path::Path) -> std::io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt as _;
    let absolute: Vec<u16> = std::path::absolute(path)?
        .as_os_str()
        .encode_wide()
        .collect();
    let mut wide = extended_length(&absolute).unwrap_or(absolute);
    wide.push(0);
    Ok(wide)
}

#[cfg(test)]
mod tests {
    use super::extended_length;

    fn extended(path: &str) -> Option<String> {
        let wide: Vec<u16> = path.encode_utf16().collect();
        extended_length(&wide).map(|out| String::from_utf16(&out).unwrap())
    }

    #[test]
    fn a_drive_path_gets_the_prefix() {
        assert_eq!(
            extended(r"C:\Users\me\Libraries\Passives").as_deref(),
            Some(r"\\?\C:\Users\me\Libraries\Passives")
        );
        assert_eq!(extended(r"d:\").as_deref(), Some(r"\\?\d:\"));
    }

    #[test]
    fn a_share_gets_the_unc_prefix() {
        assert_eq!(
            extended(r"\\server\share\libs\Passives").as_deref(),
            Some(r"\\?\UNC\server\share\libs\Passives")
        );
        assert_eq!(
            extended(r"\\server\share").as_deref(),
            Some(r"\\?\UNC\server\share")
        );
    }

    #[test]
    fn separators_dots_and_doubled_separators_are_normalised() {
        assert_eq!(
            extended(r"C:/Users//me\.\Libraries\old\..\Passives\").as_deref(),
            Some(r"\\?\C:\Users\me\Libraries\Passives")
        );
    }

    #[test]
    fn dot_dot_never_climbs_past_the_root_or_the_share() {
        assert_eq!(extended(r"C:\..\..\libs").as_deref(), Some(r"\\?\C:\libs"));
        assert_eq!(
            extended(r"\\server\share\..\..\libs").as_deref(),
            Some(r"\\?\UNC\server\share\libs")
        );
    }

    #[test]
    fn a_prefixed_or_device_path_is_left_as_it_is() {
        for path in [r"\\?\C:\a\..\b", r"\\?\UNC\server\share\a", r"\\.\pipe\x"] {
            assert_eq!(extended(path).as_deref(), Some(path));
        }
    }

    #[test]
    fn a_path_that_is_not_absolute_has_no_extended_form() {
        for path in [r"libs\Passives", r"C:libs", r"\libs", "", r"\\server"] {
            assert_eq!(extended(path), None, "{path}");
        }
    }

    #[test]
    fn a_long_path_keeps_every_character() {
        let deep = format!(r"C:\{}\Passives", "d".repeat(300));
        assert_eq!(extended(&deep), Some(format!(r"\\?\{deep}")));
    }
}
