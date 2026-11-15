//! Specifies the name the owner gives this device (#121): saved beside the database, read back,
//! and refused when blank or too long.

use super::device_name_settings::{load_saved_device_name, save_device_name};

fn database() -> (std::path::PathBuf, std::path::PathBuf) {
    let folder = std::env::temp_dir().join(format!("device-name-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
    (folder.join("nt.sqlite3"), folder)
}

#[test]
fn a_saved_name_is_read_back_and_none_is_none() {
    let (database_path, folder) = database();
    assert_eq!(load_saved_device_name(&database_path), None);

    let saved = save_device_name(&database_path, "  Ettore's phone ")
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(saved.as_str(), "Ettore's phone");
    assert_eq!(
        load_saved_device_name(&database_path).map(|name| name.as_str().to_owned()),
        Some(String::from("Ettore's phone"))
    );
    drop(std::fs::remove_dir_all(folder));
}

#[test]
fn a_blank_or_overlong_name_is_refused_and_nothing_is_saved() {
    let (database_path, folder) = database();
    assert!(save_device_name(&database_path, "   ").is_err());
    assert!(save_device_name(&database_path, &"x".repeat(200)).is_err());
    assert_eq!(load_saved_device_name(&database_path), None);
    drop(std::fs::remove_dir_all(folder));
}
