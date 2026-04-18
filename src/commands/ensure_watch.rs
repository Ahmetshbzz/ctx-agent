use super::*;

pub(super) fn cmd_ensure_watch(root: &Path, json_mode: bool) -> Result<()> {
    if !Database::exists(root) {
        super::init::cmd_init(root, json_mode)?;
    }

    watcher::ensure_background_watch(root)?;

    if json_mode {
        println!(
            "{}",
            json!({
                "command": "ensure-watch",
                "status": "ok",
                "watch": "ensured"
            })
        );
    } else {
        println!("  {} Background watch ensured", "OK".green());
    }

    Ok(())
}
