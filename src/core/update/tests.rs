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
    fn release_key_parses() {
        minisign_verify::PublicKey::from_base64(PUBKEY.unwrap()).unwrap();
    }

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
