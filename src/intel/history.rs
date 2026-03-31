use std::path::Path;

use serde::{Deserialize, Serialize};

use super::HistoryRef;

const DEFAULT_COMMIT_LIMIT: usize = 200;
const DEFAULT_TAG_LIMIT: usize = 20;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistorySnapshot {
    pub head_rev: Option<String>,
    pub recent_commits: Vec<HistoryRef>,
    pub recent_tags: Vec<HistoryRef>,
}

impl HistorySnapshot {
    pub fn read(root: &Path) -> Self {
        read_git_history(root, DEFAULT_COMMIT_LIMIT, DEFAULT_TAG_LIMIT).unwrap_or_default()
    }

    pub fn refs_for_rule(&self, rule_id: &str) -> Vec<HistoryRef> {
        let mut refs = Vec::new();

        if matches!(rule_id, "release_workflow" | "changelog") {
            refs.extend(self.recent_tags.iter().cloned());
        }

        if matches!(rule_id, "readme" | "contributing_guide" | "security_policy") {
            refs.extend(self.recent_commits.iter().take(5).cloned());
        } else {
            refs.extend(self.recent_commits.iter().take(3).cloned());
        }

        refs.truncate(8);
        refs
    }
}

fn read_git_history(root: &Path, commit_limit: usize, tag_limit: usize) -> Option<HistorySnapshot> {
    let repo = gix::discover(root).ok()?;
    let mut head = repo.head().ok()?;
    let head_id = head
        .try_peel_to_id()
        .ok()
        .flatten()
        .map(|id| id.detach().to_string());

    let recent_tags = repo
        .references()
        .ok()?
        .tags()
        .ok()?
        .take(tag_limit)
        .filter_map(Result::ok)
        .map(|reference| HistoryRef {
            kind: String::from("tag"),
            id: reference.name().as_bstr().to_string(),
            summary: format!("tag {}", reference.name().as_bstr()),
            commit_time: None,
            path: None,
        })
        .collect::<Vec<_>>();

    let recent_commits = match head.try_peel_to_id().ok().flatten() {
        Some(id) => repo
            .rev_walk([id.detach()])
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
            ))
            .all()
            .ok()?
            .take(commit_limit)
            .filter_map(Result::ok)
            .map(|info| {
                let id = info.id.to_string();
                HistoryRef {
                    kind: String::from("commit"),
                    summary: format!("commit {}", short_id(&id)),
                    id,
                    commit_time: info.commit_time,
                    path: None,
                }
            })
            .collect::<Vec<_>>(),
        None => Vec::new(),
    };

    Some(HistorySnapshot {
        head_rev: head_id,
        recent_commits,
        recent_tags,
    })
}

fn short_id(value: &str) -> &str {
    value.get(..8).unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_repo(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp directory");
        path
    }

    #[test]
    fn missing_repo_history_returns_default() {
        let root = temp_repo("ossify-history-empty");
        let history = HistorySnapshot::read(&root);
        assert!(history.head_rev.is_none());
        assert!(history.recent_commits.is_empty());
        assert!(history.recent_tags.is_empty());
        let _ = fs::remove_dir_all(&root);
    }
}
