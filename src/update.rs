//! Update check against GitHub releases, over the system `curl` so the app
//! carries no HTTP/TLS stack. A download is only installed when
//! `SHA256SUMS` carries a valid minisign signature for [`PUBKEY`] and the
//! artifact matches its line there; otherwise the release page is offered.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sha2::{Digest, Sha256};

const REPO: &str = "https://github.com/fireflylabss/abstract";
/// Minisign public key (the base64 line of `minisign.pub`) that signs
/// `SHA256SUMS` in the release workflow. `None` keeps updates notify-only.
const PUBKEY: Option<&str> = None;
const NULL_DEVICE: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
pub(crate) const EVERY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Release {
    pub version: String,
}

impl Release {
    pub(crate) fn page(&self) -> String {
        format!("{REPO}/releases/tag/v{}", self.version)
    }
}

/// How this copy was installed, and so what an update replaces.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Install {
    AppImage(PathBuf),
    MacApp(PathBuf),
    WindowsExe(PathBuf),
    Tarball(PathBuf),
    /// Package manager (.deb, .rpm, AUR), a dev build, or no signing key.
    Manual,
}

impl Install {
    pub(crate) fn detect() -> Self {
        if PUBKEY.is_none() || cfg!(debug_assertions) {
            return Self::Manual;
        }
        let Ok(exe) = std::env::current_exe().and_then(std::fs::canonicalize) else {
            return Self::Manual;
        };
        if cfg!(target_os = "linux")
            && let Some(image) = std::env::var_os("APPIMAGE")
        {
            return Self::AppImage(PathBuf::from(image));
        }
        detect_from(&exe)
    }

    pub(crate) fn automatic(&self) -> bool {
        !matches!(self, Self::Manual)
    }
}

fn detect_from(exe: &Path) -> Install {
    if cfg!(target_os = "macos") {
        return exe
            .ancestors()
            .find(|p| p.extension().is_some_and(|e| e == "app"))
            .map_or(Install::Manual, |app| Install::MacApp(app.to_path_buf()));
    }
    if cfg!(windows) {
        return Install::WindowsExe(exe.to_path_buf());
    }
    if exe.starts_with("/usr") || exe.starts_with("/opt") || exe.starts_with("/nix") {
        return Install::Manual;
    }
    Install::Tarball(exe.to_path_buf())
}

/// Release asset for `version` that replaces `install`, per
/// `scripts/package.sh` naming.
fn asset(version: &str, install: &Install) -> Option<String> {
    let arch = std::env::consts::ARCH;
    Some(match install {
        Install::AppImage(_) => format!("abstract-{version}-linux-{arch}.AppImage"),
        Install::MacApp(_) => format!("abstract-{version}-macos-{arch}.app.zip"),
        Install::WindowsExe(_) => format!("abstract-{version}-windows-{arch}.exe"),
        Install::Tarball(_) => format!("abstract-{version}-linux-{arch}.tar.gz"),
        Install::Manual => return None,
    })
}

/// `a.b.c` compared numerically; anything with a pre-release suffix or
/// that fails to parse never counts as newer.
pub(crate) fn is_newer(current: &str, latest: &str) -> bool {
    fn parse(v: &str) -> Option<(u64, u64, u64)> {
        let mut it = v.split('.').map(|p| p.parse::<u64>().ok());
        let r = (it.next()??, it.next()??, it.next()??);
        it.next().is_none().then_some(r)
    }
    match (parse(current), parse(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

/// `…/releases/tag/v0.1.3` → `0.1.3`.
fn version_from_url(url: &str) -> Option<String> {
    let tag = url.trim().rsplit_once("/releases/tag/")?.1;
    let v = tag.strip_prefix('v').unwrap_or(tag);
    (!v.is_empty()).then(|| v.to_owned())
}

fn curl() -> Command {
    let mut cmd = Command::new("curl");
    cmd.args(["--fail", "--silent", "--show-error", "--location"])
        .args(["--proto", "=https", "--max-time", "600"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn run(cmd: &mut Command) -> Result<Vec<u8>, String> {
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

/// Blocking. Latest non-prerelease, via the redirect GitHub serves for
/// `/releases/latest` (no API token or rate-limited JSON).
pub(crate) fn latest() -> Result<Release, String> {
    let url = run(curl()
        .args([
            "--head",
            "--output",
            NULL_DEVICE,
            "--write-out",
            "%{url_effective}",
        ])
        .arg(format!("{REPO}/releases/latest")))?;
    version_from_url(&String::from_utf8_lossy(&url))
        .map(|version| Release { version })
        .ok_or_else(|| "no release tag in redirect".into())
}

/// `<hex>  [dist/]<name>` line for `name`.
fn expected_hash<'a>(sums: &'a str, name: &str) -> Option<&'a str> {
    sums.lines().find_map(|l| {
        let (hash, file) = l.split_once(char::is_whitespace)?;
        let file = file.trim_start().trim_start_matches('*');
        let base = file.rsplit(['/', '\\']).next()?;
        (base == name).then_some(hash)
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn verify(pubkey: &str, sums: &[u8], sig: &str, name: &str, file: &[u8]) -> Result<(), String> {
    let key = minisign_verify::PublicKey::from_base64(pubkey).map_err(|e| e.to_string())?;
    let sig = minisign_verify::Signature::decode(sig).map_err(|e| e.to_string())?;
    key.verify(sums, &sig, false)
        .map_err(|_| "SHA256SUMS signature does not match".to_owned())?;
    let sums = std::str::from_utf8(sums).map_err(|e| e.to_string())?;
    let want =
        expected_hash(sums, name).ok_or_else(|| format!("{name} missing from SHA256SUMS"))?;
    if want.eq_ignore_ascii_case(&sha256_hex(file)) {
        Ok(())
    } else {
        Err(format!("{name} does not match SHA256SUMS"))
    }
}

fn fetch(url: &str) -> Result<Vec<u8>, String> {
    run(curl().arg(url))
}

/// Blocking. Download, verify and swap in `release` for `install`.
/// Returns what to launch afterwards. On failure the running copy is kept.
pub(crate) fn install(release: &Release, install: &Install) -> Result<PathBuf, String> {
    let pubkey = PUBKEY.ok_or("updates are not signed")?;
    let name = asset(&release.version, install).ok_or("no asset for this install")?;
    let base = format!("{REPO}/releases/download/v{}", release.version);
    let sums = fetch(&format!("{base}/SHA256SUMS"))?;
    let sig = fetch(&format!("{base}/SHA256SUMS.minisig"))?;
    let file = fetch(&format!("{base}/{name}"))?;
    verify(pubkey, &sums, &String::from_utf8_lossy(&sig), &name, &file)?;

    let target = match install {
        Install::AppImage(p)
        | Install::MacApp(p)
        | Install::WindowsExe(p)
        | Install::Tarball(p) => p.clone(),
        Install::Manual => return Err("no asset for this install".into()),
    };
    let dir = target.parent().ok_or("install has no parent folder")?;
    let stage = dir.join(".abstract-update");
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    let result = apply(install, &target, &stage, &name, &file);
    let _ = std::fs::remove_dir_all(&stage);
    result.map(|()| target)
}

fn apply(
    install: &Install,
    target: &Path,
    stage: &Path,
    name: &str,
    file: &[u8],
) -> Result<(), String> {
    let io = |e: std::io::Error| e.to_string();
    let download = stage.join(name);
    std::fs::write(&download, file).map_err(io)?;
    match install {
        Install::AppImage(_) => {
            make_executable(&download)?;
            std::fs::rename(&download, target).map_err(io)
        }
        Install::Tarball(_) => {
            run(Command::new("tar")
                .arg("-xzf")
                .arg(&download)
                .arg("-C")
                .arg(stage))?;
            let bin = stage.join("abstract");
            make_executable(&bin)?;
            std::fs::rename(&bin, target).map_err(io)
        }
        Install::WindowsExe(_) => {
            // A running .exe can be renamed but not overwritten.
            let old = target.with_extension("exe.old");
            let _ = std::fs::remove_file(&old);
            std::fs::rename(target, &old).map_err(io)?;
            std::fs::rename(&download, target).map_err(|e| {
                let _ = std::fs::rename(&old, target);
                e.to_string()
            })
        }
        Install::MacApp(_) => {
            run(Command::new("ditto")
                .arg("-x")
                .arg("-k")
                .arg(&download)
                .arg(stage))?;
            let fresh = stage.join("abstract.app");
            if !fresh.join("Contents/MacOS/abstract").is_file() {
                return Err("archive has no abstract.app".into());
            }
            let old = stage.join("previous.app");
            std::fs::rename(target, &old).map_err(io)?;
            std::fs::rename(&fresh, target).map_err(|e| {
                let _ = std::fs::rename(&old, target);
                e.to_string()
            })
        }
        Install::Manual => Err("no asset for this install".into()),
    }
}

fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Start the installed copy; the caller quits right after.
pub(crate) fn relaunch(target: &Path) -> std::io::Result<()> {
    if cfg!(target_os = "macos") {
        Command::new("open").arg("-n").arg(target).spawn()?;
    } else {
        Command::new(target).spawn()?;
    }
    Ok(())
}

/// Leftover from a Windows swap, removable once the new copy runs.
pub(crate) fn clean_up() {
    if cfg!(windows)
        && let Ok(exe) = std::env::current_exe()
    {
        let _ = std::fs::remove_file(exe.with_extension("exe.old"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("0.1.2", "0.1.3"));
        assert!(is_newer("0.1.9", "0.1.10"));
        assert!(is_newer("0.9.9", "1.0.0"));
        assert!(!is_newer("0.1.3", "0.1.3"));
        assert!(!is_newer("0.1.3", "0.1.2"));
        assert!(!is_newer("0.1.2", "0.1.3-rc1"));
        assert!(!is_newer("0.1.2", "garbage"));
        assert!(!is_newer("0.1.2", "0.1.3.4"));
    }

    #[test]
    fn version_comes_from_the_latest_redirect() {
        let url = "https://github.com/fireflylabss/abstract/releases/tag/v0.1.3";
        assert_eq!(version_from_url(url).as_deref(), Some("0.1.3"));
        let latest = "https://github.com/fireflylabss/abstract/releases/latest";
        assert_eq!(version_from_url(latest), None);
    }

    #[test]
    fn sums_lines_match_by_file_name() {
        let sums = "aa11  dist/abstract-0.1.3-linux-x86_64.tar.gz\n\
                    bb22 *abstract-0.1.3-windows-x86_64.exe\n";
        assert_eq!(
            expected_hash(sums, "abstract-0.1.3-linux-x86_64.tar.gz"),
            Some("aa11")
        );
        assert_eq!(
            expected_hash(sums, "abstract-0.1.3-windows-x86_64.exe"),
            Some("bb22")
        );
        assert_eq!(
            expected_hash(sums, "abstract-0.1.3-linux-x86_64.AppImage"),
            None
        );
    }

    // Throwaway key; `SUMS` lists sha256("abc") and is signed by it.
    const KEY: &str = "RWS8t48ZyWicYk8hX2dfKiY4JWUSM+SxTaTJr8XoJWFp/9nmo/bq9ipU";
    const SUMS: &[u8] =
        b"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  dist/abstract-0.1.3-linux-x86_64.tar.gz\n";
    const SIG: &str = "untrusted comment: signature from minisign secret key
RUS8t48ZyWicYp6pt1x9D0xXNyQQlWLXzVzhFrlHfI+i2lsqiOeaSD5b4h4apnwY+vSfFVaoeECUTsS+tGR53WC1HGu4x/DKogc=
trusted comment: abstract test fixture
iiB8GMTUXh0z0f/SwqrHrMyZs7l+DLP6bS3uTiSjnBLk3D72O1HhTk7brjsq335vDDsREMNHqOAw89ogP0umCg==";
    const NAME: &str = "abstract-0.1.3-linux-x86_64.tar.gz";

    #[test]
    fn signature_and_hash_both_gate_the_install() {
        assert_eq!(verify(KEY, SUMS, SIG, NAME, b"abc"), Ok(()));
        assert_eq!(
            verify(KEY, SUMS, SIG, NAME, b"abd").unwrap_err(),
            format!("{NAME} does not match SHA256SUMS")
        );
        let mut forged = SUMS.to_vec();
        forged[0] = b'c';
        assert!(verify(KEY, &forged, SIG, NAME, b"abc").is_err());
        let other = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
        assert!(verify(other, SUMS, SIG, NAME, b"abc").is_err());
        assert!(
            verify(
                KEY,
                SUMS,
                SIG,
                "abstract-0.1.3-linux-x86_64.AppImage",
                b"abc"
            )
            .is_err()
        );
    }

    #[test]
    #[ignore = "network"]
    fn latest_release_resolves() {
        let r = latest().unwrap();
        assert!(!is_newer(&r.version, env!("CARGO_PKG_VERSION")), "{r:?}");
    }

    #[cfg(unix)]
    #[test]
    fn tarball_install_swaps_the_binary() {
        let dir = std::env::temp_dir().join(format!("abstract-update-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let pkg = dir.join("pkg");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::write(pkg.join("abstract"), "new").unwrap();
        std::fs::write(pkg.join("README.md"), "docs").unwrap();
        let archive = dir.join("a.tar.gz");
        run(Command::new("tar")
            .arg("-C")
            .arg(&pkg)
            .arg("-czf")
            .arg(&archive)
            .arg("."))
        .unwrap();
        let target = dir.join("abstract");
        std::fs::write(&target, "old").unwrap();
        let stage = dir.join(".abstract-update");
        std::fs::create_dir_all(&stage).unwrap();

        let bytes = std::fs::read(&archive).unwrap();
        apply(
            &Install::Tarball(target.clone()),
            &target,
            &stage,
            NAME,
            &bytes,
        )
        .unwrap();

        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o755
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sha256_is_lowercase_hex() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn package_manager_installs_are_left_alone() {
        if cfg!(target_os = "linux") {
            assert_eq!(detect_from(Path::new("/usr/bin/abstract")), Install::Manual);
            assert_eq!(
                detect_from(Path::new("/home/me/bin/abstract")),
                Install::Tarball(PathBuf::from("/home/me/bin/abstract"))
            );
        }
        if cfg!(target_os = "macos") {
            assert_eq!(
                detect_from(Path::new(
                    "/Applications/abstract.app/Contents/MacOS/abstract"
                )),
                Install::MacApp(PathBuf::from("/Applications/abstract.app"))
            );
            assert_eq!(
                detect_from(Path::new("/opt/homebrew/bin/abstract")),
                Install::Manual
            );
        }
    }
}
