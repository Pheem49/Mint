pub mod branches;
pub mod checkpoint;
pub mod diff;

pub use branches::{
    BranchChange, BranchChangeOutcome, BranchInfo, change_branch, checkout_remote_branch,
    create_branch, read_branch_info, read_graph, switch_branch,
};
pub use checkpoint::{
    Checkpoint, commit_task_changes, create_checkpoint, create_task_branch,
    detect_project_language, generate_commit_message, get_current_branch, get_head_hash,
    is_git_repo, list_checkpoints, record_checkpoint, restore_file, rollback_checkpoint,
    rollback_task_changes, rollback_to_step, undo_rollback,
};
pub use diff::{DiffHunk, WorkspaceFileChange, read_workspace_git_diff};

