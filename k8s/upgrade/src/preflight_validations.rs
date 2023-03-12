use crate::user_prompt;
use operators::upgrade::{common::error, controller::reconciler};

/// Validation to be done before applying upgrade.
pub async fn preflight_check() -> Result<(), error::Error> {
    console_logger::info(user_prompt::UPGRADE_WARNING);
    rebuild_in_progress_validation().await?;
    Ok(())
}

/// Check for rebuild in progress.
pub async fn rebuild_in_progress_validation() -> Result<(), error::Error> {
    match reconciler::is_rebuilding().await {
        Ok(is_rebuilding) => {
            if is_rebuilding {
                console_logger::warn(user_prompt::REBUILD_WARNING, "");
                std::process::exit(1);
            }
        }
        Err(error) => {
            println!("Failed in fetching the rebuild status {error}");
            return Err(error);
        }
    }
    Ok(())
}
