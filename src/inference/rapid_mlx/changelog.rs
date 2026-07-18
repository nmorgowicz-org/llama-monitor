use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde::Deserialize;

const CHANGELOG_CACHE_TTL: Duration = Duration::from_secs(300);
const MAX_RESPONSE_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChangelogSummary {
    pub commits: Vec<CommitEntry>,
    pub html_url: String,
    pub ahead_by: usize,
    pub behind_by: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CommitEntry {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub html_url: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct CompareResponse {
    html_url: String,
    ahead_by: usize,
    behind_by: usize,
    commits: Vec<CommitItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct CommitItem {
    sha: String,
    commit: CommitData,
    html_url: String,
    author: Option<GithubUser>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct CommitData {
    message: String,
    author: CommitAuthor,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct CommitAuthor {
    name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct GithubUser {
    login: String,
}

#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangelogErrorKind {
    RateLimited,
    NetworkFailure,
    InvalidTag,
    InternalError,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChangelogError {
    pub kind: ChangelogErrorKind,
    pub message: String,
}

type CacheEntry = (Instant, Result<ChangelogSummary, ChangelogError>);
type ChangelogCache = HashMap<String, CacheEntry>;

pub struct ChangelogCacheManager {
    inner: Mutex<ChangelogCache>,
}

impl ChangelogCacheManager {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, from: &str, to: &str) -> Option<CacheEntry> {
        let key = cache_key(from, to);
        let cache = self.inner.lock().ok()?;
        cache.get(&key).cloned()
    }

    pub fn set(&self, from: &str, to: &str, value: Result<ChangelogSummary, ChangelogError>) {
        let key = cache_key(from, to);
        if let Ok(mut cache) = self.inner.lock() {
            cache.insert(key, (Instant::now(), value));
        }
    }

    pub fn prune_expired(&self) {
        if let Ok(mut cache) = self.inner.lock() {
            cache.retain(|_, (instant, _)| instant.elapsed() < CHANGELOG_CACHE_TTL);
        }
    }
}

impl Default for ChangelogCacheManager {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn fetch_compare(
    client: &reqwest::Client,
    cache: &ChangelogCacheManager,
    base_version: &str,
    head_version: &str,
) -> Result<ChangelogSummary, ChangelogError> {
    if base_version.is_empty() || head_version.is_empty() {
        return Err(ChangelogError {
            kind: ChangelogErrorKind::InvalidTag,
            message: "Both from and to versions are required".to_string(),
        });
    }

    if base_version == head_version {
        return Err(ChangelogError {
            kind: ChangelogErrorKind::InvalidTag,
            message: "from and to versions are identical".to_string(),
        });
    }

    if let Some((instant, cached)) = cache.get(base_version, head_version)
        && instant.elapsed() < CHANGELOG_CACHE_TTL
    {
        return cached.clone();
    }

    cache.prune_expired();

    let result = fetch_compare_uncached(client, base_version, head_version).await;
    cache.set(base_version, head_version, result.clone());
    result
}

fn cache_key(from: &str, to: &str) -> String {
    format!("{}...{}", from, to)
}

async fn fetch_compare_uncached(
    client: &reqwest::Client,
    base_version: &str,
    head_version: &str,
) -> Result<ChangelogSummary, ChangelogError> {
    let base_tag = format!("v{base_version}");
    let head_tag = format!("v{head_version}");
    let url = format!(
        "https://api.github.com/repos/raullenchai/Rapid-MLX/compare/{base_tag}...{head_tag}"
    );

    let response = client.get(&url).send().await.map_err(|e| ChangelogError {
        kind: ChangelogErrorKind::NetworkFailure,
        message: format!("Failed to contact GitHub API: {e}"),
    })?;

    let status = response.status();

    if status.as_u16() == 403 || status.as_u16() == 429 {
        let remaining = response
            .headers()
            .get(reqwest::header::HeaderName::from_static(
                "x-ratelimit-remaining",
            ))
            .and_then(|v| v.to_str().ok())
            .unwrap_or("0")
            .to_string();

        return Err(ChangelogError {
            kind: ChangelogErrorKind::RateLimited,
            message: format!(
                "Changelog unavailable: GitHub API rate-limited (remaining: {remaining})"
            ),
        });
    }

    if status.as_u16() == 404 {
        return Err(ChangelogError {
            kind: ChangelogErrorKind::InvalidTag,
            message: "One or both version tags were not found on GitHub".to_string(),
        });
    }

    if !status.is_success() {
        return Err(ChangelogError {
            kind: ChangelogErrorKind::InternalError,
            message: format!("GitHub API returned HTTP {status}: changelog unavailable"),
        });
    }

    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ChangelogError {
            kind: ChangelogErrorKind::NetworkFailure,
            message: format!("Failed to read GitHub API response: {e}"),
        })?;
        if body.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(ChangelogError {
                kind: ChangelogErrorKind::InternalError,
                message: "GitHub API response exceeded size limit".to_string(),
            });
        }
        body.extend_from_slice(&chunk);
    }

    let compare: CompareResponse = serde_json::from_slice(&body).map_err(|e| ChangelogError {
        kind: ChangelogErrorKind::InternalError,
        message: format!("Failed to parse GitHub API response: {e}"),
    })?;

    let commits = summarize_commits(compare.commits);

    Ok(ChangelogSummary {
        commits,
        html_url: compare.html_url,
        ahead_by: compare.ahead_by,
        behind_by: compare.behind_by,
    })
}

fn summarize_commits(raw: Vec<CommitItem>) -> Vec<CommitEntry> {
    const MAX_COMMITS: usize = 250;

    raw.into_iter()
        .take(MAX_COMMITS)
        .map(|item| {
            let subject = extract_subject(&item.commit.message);
            let login = item
                .author
                .as_ref()
                .map(|a| a.login.clone())
                .unwrap_or_default();
            let author = if !login.is_empty() {
                login
            } else if !item.commit.author.name.is_empty() {
                item.commit.author.name
            } else {
                "unknown".to_string()
            };

            CommitEntry {
                sha: item.sha.chars().take(7).collect(),
                message: subject,
                author,
                html_url: item.html_url,
            }
        })
        .collect()
}

fn extract_subject(message: &str) -> String {
    message.lines().next().unwrap_or(message).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_subject_strips_body_and_coauthored() {
        let msg = "feat: add pflash compression (#1106)\n\nSome body text here.\n\nCo-authored-by: Someone <x@y.z>";
        assert_eq!(extract_subject(msg), "feat: add pflash compression (#1106)");
    }

    #[test]
    fn extract_subject_handles_empty() {
        assert_eq!(extract_subject(""), "");
    }

    #[test]
    fn extract_subject_handles_single_line() {
        assert_eq!(extract_subject("fix: hotfix"), "fix: hotfix");
    }

    #[tokio::test]
    async fn fetch_compare_rejects_empty_versions() {
        let cache = ChangelogCacheManager::new();
        let client = reqwest::Client::new();
        let result = fetch_compare(&client, &cache, "", "0.10.12").await;
        assert!(matches!(
            result,
            Err(ChangelogError {
                kind: ChangelogErrorKind::InvalidTag,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn fetch_compare_rejects_identical_versions() {
        let cache = ChangelogCacheManager::new();
        let client = reqwest::Client::new();
        let result = fetch_compare(&client, &cache, "0.10.10", "0.10.10").await;
        assert!(matches!(
            result,
            Err(ChangelogError {
                kind: ChangelogErrorKind::InvalidTag,
                ..
            })
        ));
    }

    #[test]
    fn cache_key_is_deterministic() {
        assert_eq!(cache_key("0.10.10", "0.10.12"), "0.10.10...0.10.12");
    }

    #[test]
    fn cache_get_returns_none_for_missing_key() {
        let cache = ChangelogCacheManager::new();
        assert!(cache.get("0.10.10", "0.10.12").is_none());
    }

    #[test]
    fn cache_respects_ttl() {
        let cache = ChangelogCacheManager::new();
        cache.set(
            "a",
            "b",
            Ok(ChangelogSummary {
                commits: vec![],
                html_url: "x".into(),
                ahead_by: 0,
                behind_by: 0,
            }),
        );
        assert!(cache.get("a", "b").is_some());
    }

    #[test]
    fn cache_prune_expired_removes_stale_entries() {
        let cache = ChangelogCacheManager::new();
        if let Ok(mut inner) = cache.inner.lock() {
            inner.insert(
                "old".into(),
                (
                    Instant::now() - Duration::from_secs(600),
                    Ok(ChangelogSummary {
                        commits: vec![],
                        html_url: "x".into(),
                        ahead_by: 0,
                        behind_by: 0,
                    }),
                ),
            );
            inner.insert(
                "new".into(),
                (
                    Instant::now(),
                    Ok(ChangelogSummary {
                        commits: vec![],
                        html_url: "y".into(),
                        ahead_by: 0,
                        behind_by: 0,
                    }),
                ),
            );
        }
        cache.prune_expired();
        if let Ok(inner) = cache.inner.lock() {
            assert!(inner.contains_key("new"));
            assert!(!inner.contains_key("old"));
        }
    }

    #[test]
    fn summarize_commits_maps_real_github_shape() {
        let raw = vec![CommitItem {
            sha: "abd3f09c927803e6d37c63e65b6324a15fd48bff".into(),
            commit: CommitData {
                message: "fix: exclude broken mlx-vlm 0.6.4 from all extras (#1119)\n\nLong description...\n\nCo-authored-by: X <x@y.z>".into(),
                author: CommitAuthor {
                    name: "Raullen Chai".into(),
                },
            },
            html_url: "https://github.com/raullenchai/Rapid-MLX/commit/abd3f09c927803e6d37c63e65b6324a15fd48bff".into(),
            author: Some(GithubUser {
                login: "raullenchai".into(),
            }),
        }];

        let entries = summarize_commits(raw);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].sha, "abd3f09");
        assert_eq!(
            entries[0].message,
            "fix: exclude broken mlx-vlm 0.6.4 from all extras (#1119)"
        );
        assert_eq!(entries[0].author, "raullenchai");
    }

    #[test]
    fn summarize_commits_limits_to_250() {
        let raw: Vec<CommitItem> = (0..300)
            .map(|i| CommitItem {
                sha: format!("{:040x}", i),
                commit: CommitData {
                    message: format!("commit {i}"),
                    author: CommitAuthor {
                        name: "tester".into(),
                    },
                },
                html_url: format!("https://example.com/{i}"),
                author: Some(GithubUser {
                    login: "tester".into(),
                }),
            })
            .collect();

        let entries = summarize_commits(raw);
        assert_eq!(entries.len(), 250);
    }

    #[test]
    fn summarize_commits_falls_back_to_author_name_when_login_missing() {
        let raw = vec![CommitItem {
            sha: "abcdef1".into(),
            commit: CommitData {
                message: "test commit".into(),
                author: CommitAuthor {
                    name: "Real Author".into(),
                },
            },
            html_url: "https://example.com".into(),
            author: Some(GithubUser { login: "".into() }),
        }];

        let entries = summarize_commits(raw);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].author, "Real Author");
    }

    #[test]
    fn changelog_summary_serializes_cleanly() {
        let summary = ChangelogSummary {
            commits: vec![CommitEntry {
                sha: "abc1234".into(),
                message: "feat: new thing".into(),
                author: "tester".into(),
                html_url: "https://github.com/x/y/commit/abc1234".into(),
            }],
            html_url: "https://github.com/x/y/compare/a...b".into(),
            ahead_by: 5,
            behind_by: 0,
        };

        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"ahead_by\":5"));
        assert!(json.contains("abc1234"));
    }

    #[test]
    fn real_github_compare_api_fixture_decodes_correctly() {
        let fixture = br#"
        {
          "url": "https://api.github.com/repos/raullenchai/Rapid-MLX/compare/v0.10.10...v0.10.12",
          "html_url": "https://github.com/raullenchai/Rapid-MLX/compare/v0.10.10...v0.10.12",
          "status": "ahead",
          "ahead_by": 9,
          "behind_by": 0,
          "total_commits": 9,
          "commits": [
            {
              "sha": "abd3f09c927803e6d37c63e65b6324a15fd48bff",
              "commit": {
                "author": { "name": "Raullen Chai", "email": "raullenchai@gmail.com", "date": "2026-07-16T03:58:12Z" },
                "committer": { "name": "GitHub", "email": "noreply@github.com", "date": "2026-07-16T03:58:12Z" },
                "message": "fix: exclude broken mlx-vlm 0.6.4 from all extras (#1119)\n\nmlx-vlm 0.6.4 ships a broken qwen3_5 GatedDeltaNet SSM forward",
                "tree": { "sha": "eb0d758a38e1dcc46141ef4365d248e8a7ccaedc" },
                "url": "https://api.github.com/repos/raullenchai/Rapid-MLX/git/commits/abd3f09c927803e6d37c63e65b6324a15fd48bff",
                "comment_count": 0,
                "verification": { "verified": true, "reason": "valid" }
              },
              "url": "https://api.github.com/repos/raullenchai/Rapid-MLX/commits/abd3f09c927803e6d37c63e65b6324a15fd48bff",
              "html_url": "https://github.com/raullenchai/Rapid-MLX/commit/abd3f09c927803e6d37c63e65b6324a15fd48bff",
              "author": { "login": "raullenchai" },
              "committer": { "login": "web-flow" }
            },
            {
              "sha": "20eedd837e778c7e79a0a62ee11b32ff5c659bd7",
              "commit": {
                "author": { "name": "pierre427", "email": "pierre@userid.org", "date": "2026-07-16T12:32:22Z" },
                "committer": { "name": "GitHub", "email": "noreply@github.com", "date": "2026-07-16T12:32:22Z" },
                "message": "feat(pflash): surface silent endpoints-only compression collapse (#1106)",
                "tree": { "sha": "076325dd3fda2223348d618312c635228534cf7e" },
                "url": "https://api.github.com/repos/raullenchai/Rapid-MLX/git/commits/20eedd837e778c7e79a0a62ee11b32ff5c659bd7",
                "comment_count": 0,
                "verification": { "verified": true, "reason": "valid" }
              },
              "url": "https://api.github.com/repos/raullenchai/Rapid-MLX/commits/20eedd837e778c7e79a0a62ee11b32ff5c659bd7",
              "html_url": "https://github.com/raullenchai/Rapid-MLX/commit/20eedd837e778c7e79a0a62ee11b32ff5c659bd7",
              "author": { "login": "pierre427" },
              "committer": { "login": "web-flow" }
            }
          ]
        }
        "#;

        let response: CompareResponse = serde_json::from_slice(fixture).unwrap();
        assert_eq!(response.ahead_by, 9);
        assert_eq!(response.behind_by, 0);
        assert_eq!(response.commits.len(), 2);
        assert!(response.html_url.contains("compare/v0.10.10...v0.10.12"));

        let entries = summarize_commits(response.commits);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sha, "abd3f09");
        assert_eq!(
            entries[0].message,
            "fix: exclude broken mlx-vlm 0.6.4 from all extras (#1119)"
        );
        assert_eq!(entries[0].author, "raullenchai");
        assert_eq!(entries[1].author, "pierre427");
    }

    #[test]
    fn real_github_compare_api_fixture_with_null_author_still_deserializes() {
        // GitHub's compare API returns "author": null for commits whose commit
        // email isn't linked to a GitHub account (bot/orphaned commits). This
        // fixture mirrors the real shape and must not fail the whole response.
        let fixture = br#"
        {
          "html_url": "https://github.com/raullenchai/Rapid-MLX/compare/v0.10.10...v0.10.12",
          "ahead_by": 2,
          "behind_by": 0,
          "commits": [
            {
              "sha": "abd3f09c927803e6d37c63e65b6324a15fd48bff",
              "commit": {
                "author": { "name": "Raullen Chai", "email": "raullenchai@gmail.com", "date": "2026-07-16T03:58:12Z" },
                "committer": { "name": "GitHub", "email": "noreply@github.com", "date": "2026-07-16T03:58:12Z" },
                "message": "fix: exclude broken mlx-vlm 0.6.4 from all extras (#1119)"
              },
              "url": "https://api.github.com/repos/raullenchai/Rapid-MLX/commits/abd3f09c927803e6d37c63e65b6324a15fd48bff",
              "html_url": "https://github.com/raullenchai/Rapid-MLX/commit/abd3f09c927803e6d37c63e65b6324a15fd48bff",
              "author": { "login": "raullenchai" },
              "committer": { "login": "web-flow" }
            },
            {
              "sha": "9f1e2a3b4c5d6e7f8091a2b3c4d5e6f708192a3b",
              "commit": {
                "author": { "name": "orphaned-bot", "email": "bot@nowhere.example", "date": "2026-07-16T12:32:22Z" },
                "committer": { "name": "GitHub", "email": "noreply@github.com", "date": "2026-07-16T12:32:22Z" },
                "message": "chore: bump lockfile"
              },
              "url": "https://api.github.com/repos/raullenchai/Rapid-MLX/commits/9f1e2a3b4c5d6e7f8091a2b3c4d5e6f708192a3b",
              "html_url": "https://github.com/raullenchai/Rapid-MLX/commit/9f1e2a3b4c5d6e7f8091a2b3c4d5e6f708192a3b",
              "author": null,
              "committer": null
            }
          ]
        }
        "#;

        let response: CompareResponse = serde_json::from_slice(fixture)
            .expect("compare response with a null commit author must still deserialize");
        assert_eq!(response.commits.len(), 2);

        let entries = summarize_commits(response.commits);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].author, "raullenchai");
        // No top-level GitHub account (author: null) -> falls back to the
        // nested commit-author name, which GitHub always populates.
        assert_eq!(entries[1].author, "orphaned-bot");
    }
}
