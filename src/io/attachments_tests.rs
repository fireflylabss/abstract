use super::Dir;
use crate::core::attach::{self, Incoming};
use crate::io::attachments::{Imports, Job};
use std::time::Duration;

#[test]
fn imports_reserve_collision_safe_names_even_across_concurrent_workers() {
    let dir = Dir::new();
    let note = dir.0.join("Note.md");
    std::fs::write(&note, "note").unwrap();
    let root = dir.0.clone();
    let threads: Vec<_> = (0..8)
        .map(|n| {
            let root = root.clone();
            std::thread::spawn(move || {
                attach::import(
                    vec![Incoming::Image {
                        bytes: vec![n],
                        ext: "png",
                    }],
                    &root,
                    &root,
                    "same",
                )
                .unwrap()
            })
        })
        .collect();
    let names: std::collections::HashSet<_> = threads
        .into_iter()
        .flat_map(|t| t.join().unwrap())
        .collect();
    assert_eq!(names.len(), 8);
    let mut contents: Vec<_> = std::fs::read_dir(root.join("attachments"))
        .unwrap()
        .map(|entry| std::fs::read(entry.unwrap().path()).unwrap()[0])
        .collect();
    contents.sort();
    assert_eq!(contents, (0..8).collect::<Vec<_>>());
}

#[test]
fn imports_handle_empty_input_and_surface_missing_files_and_invalid_destinations() {
    let dir = Dir::new();
    let worker = Imports::new();
    let path = dir.0.join("Note.md");
    std::fs::write(&path, "note").unwrap();
    let run = |items| {
        worker
            .submit(Job {
                path: path.clone(),
                root: dir.0.clone(),
                items,
            })
            .unwrap();
        worker.replies.recv_timeout(Duration::from_secs(5)).unwrap()
    };
    assert!(run(vec![]).unwrap().is_empty());
    assert!(run(vec![Incoming::Path(dir.0.join("missing.png"))]).is_err());
    std::fs::write(dir.0.join("attachments"), "occupied").unwrap();
    assert!(
        run(vec![Incoming::Image {
            bytes: vec![1, 2],
            ext: "png"
        }])
        .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(dir.0.join("attachments")).unwrap(),
        "occupied"
    );
}
