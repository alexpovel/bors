use crate::github::GithubRepoName;
use crate::permissions::PermissionType;
use crate::tests::event::default_pr_number;
use base64::Engine;
use serde::Serialize;
use std::collections::HashMap;
use tokio::sync::mpsc::Sender;
use url::Url;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use crate::tests::mocks::comment::Comment;
use crate::tests::mocks::pull_request::mock_pull_requests;
use crate::tests::mocks::{Permissions, World};

use super::user::{GitHubUser, User};

#[derive(Clone)]
pub struct Repo {
    pub name: GithubRepoName,
    pub permissions: Permissions,
    pub config: String,
    // Pre-set known PRs to avoid responding to requests about PRs that we
    // don't expect.
    pub known_prs: Vec<u64>,
}

impl Repo {
    pub fn new(owner: &str, name: &str, permissions: Permissions, config: String) -> Self {
        Self {
            name: GithubRepoName::new(owner, name),
            permissions,
            config,
            known_prs: vec![default_pr_number()],
        }
    }

    pub fn perms(mut self, user: User, permissions: &[PermissionType]) -> Self {
        self.permissions.users.insert(user, permissions.to_vec());
        self
    }
}

impl Default for Repo {
    fn default() -> Self {
        let config = r#"
timeout = 3600
"#
        .to_string();
        let mut users = HashMap::default();
        users.insert(
            User::default(),
            vec![PermissionType::Try, PermissionType::Review],
        );

        Self::new(
            default_repo_name().owner(),
            default_repo_name().name(),
            Permissions { users },
            config,
        )
    }
}

fn default_repo_name() -> GithubRepoName {
    GithubRepoName::new("rust-lang", "borstest")
}

pub async fn mock_repo_list(world: &World, mock_server: &MockServer) {
    let repos = GitHubRepositories {
        total_count: world.repos.len() as u64,
        repositories: world
            .repos
            .iter()
            .enumerate()
            .map(|(index, (_, repo))| GitHubRepository {
                id: index as u64,
                owner: User::new(index as u64, repo.name.owner()).into(),
                name: repo.name.name().to_string(),
                url: format!("https://{}.foo", repo.name.name()).parse().unwrap(),
            })
            .collect(),
    };

    Mock::given(method("GET"))
        .and(path("/installation/repositories"))
        .respond_with(ResponseTemplate::new(200).set_body_json(repos))
        .mount(mock_server)
        .await;
}

pub async fn mock_repo(repo: &Repo, comments_tx: Sender<Comment>, mock_server: &MockServer) {
    mock_pull_requests(repo, mock_server).await;
    mock_config(repo, mock_server).await;
}

async fn mock_config(repo: &Repo, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!(
            "/repos/{}/contents/rust-bors.toml",
            repo.name
        )))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(GitHubContent::new("rust-bors.toml", &repo.config)),
        )
        .mount(mock_server)
        .await;
}

/// Represents all repositories for an installation
/// Returns type for the `GET /installation/repositories` endpoint
#[derive(Serialize)]
struct GitHubRepositories {
    total_count: u64,
    repositories: Vec<GitHubRepository>,
}

#[derive(Serialize)]
pub struct GitHubRepository {
    id: u64,
    name: String,
    url: Url,
    owner: GitHubUser,
}

impl From<GithubRepoName> for GitHubRepository {
    fn from(value: GithubRepoName) -> Self {
        Self {
            id: 1,
            name: value.name().to_string(),
            owner: GitHubUser::new(value.owner(), 1001),
            url: format!("https://github.com/{}", value).parse().unwrap(),
        }
    }
}

/// Represents a file in a GitHub repository
/// returns type for the `GET /repos/{owner}/{repo}/contents/{path}` endpoint
#[derive(Serialize)]
struct GitHubContent {
    name: String,
    path: String,
    sha: String,
    encoding: Option<String>,
    content: Option<String>,
    size: i64,
    url: String,
    r#type: String,
    #[serde(rename = "_links")]
    links: GitHubContentLinks,
}

impl GitHubContent {
    fn new(path: &str, content: &str) -> Self {
        let content = base64::prelude::BASE64_STANDARD.encode(content);
        let size = content.len() as i64;
        GitHubContent {
            name: path.to_string(),
            path: path.to_string(),
            sha: "test".to_string(),
            encoding: Some("base64".to_string()),
            content: Some(content),
            size,
            url: "https://test.com".to_string(),
            r#type: "file".to_string(),
            links: GitHubContentLinks {
                _self: "https://test.com".parse().unwrap(),
            },
        }
    }
}

#[derive(Serialize)]
struct GitHubContentLinks {
    #[serde(rename = "self")]
    _self: Url,
}
