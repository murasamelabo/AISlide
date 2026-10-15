#[cfg(any(windows, test))]
fn local_drive_type(drive_type: u32) -> bool {
    matches!(drive_type, 2 | 3 | 5 | 6)
}

/// Returns whether an ASCII drive letter currently has a known local drive type.
/// This classification does not protect against concurrent drive remapping.
#[cfg(windows)]
pub fn is_local_drive(letter: u8) -> bool {
    use windows::{Win32::Storage::FileSystem::GetDriveTypeW, core::PCWSTR};

    if !letter.is_ascii_alphabetic() {
        return false;
    }
    let root = [u16::from(letter), u16::from(b':'), u16::from(b'\\'), 0];
    // SAFETY: root is a valid NUL-terminated UTF-16 drive root, remains alive for
    // this synchronous call, and is read-only. No caller-provided pointer is used.
    let drive_type = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
    local_drive_type(drive_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_local_drive_types_pass() {
        for (drive_type, expected) in [
            (0, false), (1, false), (2, true), (3, true),
            (4, false), (5, true), (6, true), (7, false), (u32::MAX, false),
        ] {
            assert_eq!(local_drive_type(drive_type), expected, "DriveType {drive_type}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn invalid_drive_bytes_fail_without_querying_a_drive() {
        for letter in 0..=u8::MAX {
            if !letter.is_ascii_alphabetic() {
                assert!(!is_local_drive(letter), "invalid drive byte {letter}");
            }
        }
    }
}