use super::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(crate) struct Mock {
    pub version: Result<String, String>,
    pub calls: Arc<Mutex<Vec<&'static str>>>,
    pub fail_install: bool,
}
impl Backend for Mock {
    fn latest(&mut self) -> Result<Release, String> {
        self.calls.lock().unwrap().push("check");
        self.version.clone().map(|version| Release { version })
    }
    fn detect(&mut self) -> Install {
        Install::Tarball(PathBuf::from("abstract"))
    }
    fn install(&mut self, _: &Release, _: &Install) -> Result<PathBuf, String> {
        self.calls.lock().unwrap().push("install");
        if self.fail_install {
            Err("signature rejected".into())
        } else {
            Ok(PathBuf::from("abstract-new"))
        }
    }
    fn relaunch(&mut self, _: &std::path::Path) -> Result<(), String> {
        self.calls.lock().unwrap().push("relaunch");
        Ok(())
    }
}
pub(crate) fn mock(
    version: Result<&str, &str>,
    fail_install: bool,
) -> (Updates, Arc<Mutex<Vec<&'static str>>>) {
    let calls = Arc::new(Mutex::new(vec![]));
    let worker = Updates::with_backend(Mock {
        version: version.map(str::to_owned).map_err(str::to_owned),
        calls: calls.clone(),
        fail_install,
    });
    (worker, calls)
}
#[test]
fn mocked_transport_runs_check_install_relaunch_in_order() {
    let (worker, calls) = mock(Ok("99.0.0"), false);
    assert!(calls.lock().unwrap().is_empty());
    worker.submit(Job::Check(7)).unwrap();
    let Reply::Checked(7, Ok((r, i))) =
        worker.replies.recv_timeout(Duration::from_secs(2)).unwrap()
    else {
        panic!()
    };
    worker.submit(Job::Install(8, r, i)).unwrap();
    let Reply::Installed(8, Ok(path)) =
        worker.replies.recv_timeout(Duration::from_secs(2)).unwrap()
    else {
        panic!()
    };
    worker.submit(Job::Relaunch(9, path)).unwrap();
    assert!(matches!(
        worker.replies.recv_timeout(Duration::from_secs(2)).unwrap(),
        Reply::Relaunched(9, Ok(()))
    ));
    assert_eq!(*calls.lock().unwrap(), vec!["check", "install", "relaunch"]);
}
#[test]
fn mock_errors_are_delivered_without_network() {
    let (worker, _) = mock(Err("offline"), true);
    worker.submit(Job::Check(1)).unwrap();
    assert!(
        matches!(worker.replies.recv_timeout(Duration::from_secs(2)).unwrap(), Reply::Checked(1, Err(e)) if e == "offline")
    );
    worker
        .submit(Job::Install(
            2,
            Release {
                version: "99.0.0".into(),
            },
            Install::Manual,
        ))
        .unwrap();
    assert!(
        matches!(worker.replies.recv_timeout(Duration::from_secs(2)).unwrap(), Reply::Installed(2, Err(e)) if e == "signature rejected")
    );
}
