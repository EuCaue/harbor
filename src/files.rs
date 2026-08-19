use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::Mode;

pub fn apply(src: &Path, dest_dir: &Path, mode: Mode, dedup: bool) -> io::Result<PathBuf> {
    fs::create_dir_all(dest_dir)?;
    let dst = dedup_path(&dest_dir.join(file_name(src)), dedup);
    match mode {
        Mode::Move => move_file(src, &dst)?,
        Mode::Copy => {
            fs::copy(src, &dst)?;
            fs::set_permissions(&dst, fs::metadata(src)?.permissions())?;
        }
    }
    Ok(dst)
}

fn move_file(src: &Path, dst: &Path) -> io::Result<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        // Cross-device (ex: /mnt/usb -> ~/Pictures): rename can't, copy+remove instead.
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices => {
            fs::copy(src, dst)?;
            fs::set_permissions(dst, fs::metadata(src)?.permissions())?;
            fs::remove_file(src)
        }
        Err(e) => Err(e),
    }
}

fn dedup_path(dst: &Path, dedup: bool) -> PathBuf {
    if !dst.exists() || !dedup {
        return dst.to_path_buf();
    }
    let parent = dst.parent().unwrap_or(Path::new("."));
    let stem = dst.file_stem().unwrap_or_default().to_string_lossy();
    let ext = dst
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut n = 1;
    loop {
        let cand = parent.join(format!("{}_{}{}", stem, n, ext));
        if !cand.exists() {
            return cand;
        }
        n += 1;
    }
}

fn file_name(p: &Path) -> PathBuf {
    p.file_name().unwrap_or_default().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("harbor-files-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn move_same_fs() {
        let d = tmpdir("move");
        let src = d.join("a.txt");
        let dst_dir = d.join("dst");
        File::create(&src).unwrap();
        let moved = apply(&src, &dst_dir, Mode::Move, true).unwrap();
        assert_eq!(moved, dst_dir.join("a.txt"));
        assert!(dst_dir.join("a.txt").is_file());
        assert!(!src.exists());
    }

    #[test]
    fn copy_keeps_source_and_perms() {
        let d = tmpdir("copy");
        let src = d.join("a.txt");
        let dst_dir = d.join("dst");
        fs::write(&src, b"x").unwrap();
        let mut perms = fs::metadata(&src).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&src, perms).unwrap();

        let moved = apply(&src, &dst_dir, Mode::Copy, true).unwrap();
        assert!(src.exists()); // copy keeps source
        assert!(moved.is_file());
        assert!(fs::metadata(&moved).unwrap().permissions().readonly());
    }

    #[test]
    fn dedup_suffixes_on_conflict() {
        let d = tmpdir("dedup");
        let dst_dir = d.join("dst");
        fs::create_dir_all(&dst_dir).unwrap();
        fs::write(dst_dir.join("a.txt"), b"1").unwrap();

        let src = d.join("a.txt");
        fs::write(&src, b"2").unwrap();
        let first = apply(&src, &dst_dir, Mode::Move, true).unwrap();
        fs::write(&src, b"3").unwrap();
        let second = apply(&src, &dst_dir, Mode::Move, true).unwrap();
        assert_eq!(first, dst_dir.join("a_1.txt"));
        assert_eq!(second, dst_dir.join("a_2.txt"));
    }

    #[test]
    fn dedup_disabled_overwrites() {
        let d = tmpdir("overwrite");
        let dst_dir = d.join("dst");
        fs::create_dir_all(&dst_dir).unwrap();
        fs::write(dst_dir.join("a.txt"), b"old").unwrap();
        let src = d.join("a.txt");
        fs::write(&src, b"new").unwrap();

        let moved = apply(&src, &dst_dir, Mode::Move, false).unwrap();
        assert_eq!(moved, dst_dir.join("a.txt"));
        assert_eq!(fs::read_to_string(moved).unwrap(), "new");
    }

    #[test]
    fn dedup_handles_multiple_extensions() {
        let d = tmpdir("ext");
        let dst_dir = d.join("dst");
        fs::create_dir_all(&dst_dir).unwrap();
        fs::write(dst_dir.join("a.tar.gz"), b"1").unwrap();
        let src = d.join("a.tar.gz");
        fs::write(&src, b"2").unwrap();
        let moved = apply(&src, &dst_dir, Mode::Move, true).unwrap();
        // stem-based suffix keeps last extension intact: a.tar.gz -> a.tar_1.gz
        assert_eq!(moved, dst_dir.join("a.tar_1.gz"));
    }
}
