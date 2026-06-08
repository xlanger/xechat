use tempfile::TempDir;

pub fn setup_temp_dir() -> TempDir {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let path = temp.path().to_path_buf();
    xechat::services::paths::set_test_dir(path);
    temp
}
