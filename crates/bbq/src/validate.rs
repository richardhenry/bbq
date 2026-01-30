pub fn validate_worktree_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Worktree name required".to_string());
    }
    if name.chars().any(|ch| ch.is_whitespace()) {
        return Err("Worktree name cannot contain spaces".to_string());
    }
    if name.chars().any(|ch| !is_worktree_char(ch)) {
        return Err("Worktree name can only use letters, numbers, '-', '_', or '.'".to_string());
    }
    Ok(())
}

pub fn validate_branch_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Branch name required".to_string());
    }
    if name.chars().any(|ch| ch.is_whitespace()) {
        return Err("Branch name cannot contain spaces".to_string());
    }
    if name.starts_with('/') || name.ends_with('/') {
        return Err("Branch name cannot start or end with '/'".to_string());
    }
    if name.chars().any(|ch| !is_branch_char(ch)) {
        return Err("Branch name can only use letters, numbers, '-', '_', '.', or '/'".to_string());
    }
    Ok(())
}

fn is_worktree_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.'
}

fn is_branch_char(ch: char) -> bool {
    is_worktree_char(ch) || ch == '/'
}

#[cfg(test)]
mod tests {
    use super::{validate_branch_name, validate_worktree_name};

    #[test]
    fn validate_worktree_name_rejects_invalid() {
        assert_eq!(
            validate_worktree_name(""),
            Err("Worktree name required".to_string())
        );
        assert_eq!(
            validate_worktree_name("bad name"),
            Err("Worktree name cannot contain spaces".to_string())
        );
        assert_eq!(
            validate_worktree_name("bad@name"),
            Err("Worktree name can only use letters, numbers, '-', '_', or '.'".to_string())
        );
    }

    #[test]
    fn validate_worktree_name_accepts_valid() {
        assert_eq!(validate_worktree_name("feature-1.2_ok"), Ok(()));
    }

    #[test]
    fn validate_branch_name_rejects_invalid() {
        assert_eq!(
            validate_branch_name(""),
            Err("Branch name required".to_string())
        );
        assert_eq!(
            validate_branch_name("bad name"),
            Err("Branch name cannot contain spaces".to_string())
        );
        assert_eq!(
            validate_branch_name("/bad"),
            Err("Branch name cannot start or end with '/'".to_string())
        );
        assert_eq!(
            validate_branch_name("bad/"),
            Err("Branch name cannot start or end with '/'".to_string())
        );
        assert_eq!(
            validate_branch_name("bad@name"),
            Err("Branch name can only use letters, numbers, '-', '_', '.', or '/'".to_string())
        );
    }

    #[test]
    fn validate_branch_name_accepts_valid() {
        assert_eq!(validate_branch_name("feature/test-1.2_ok"), Ok(()));
    }
}
