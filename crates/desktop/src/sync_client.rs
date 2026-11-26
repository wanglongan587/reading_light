use anyhow::{Context, Result, anyhow, ensure};
use base64::Engine as _;
use glow_core::{
    State, SyncSettings,
    model::now,
    store::atomic_write,
    sync::{MergeReport, SyncSnapshot, merge_state_with_snapshot},
};
use serde_json::{Value, json};
use std::{fs, path::Path, time::Duration};

const SNAPSHOT_PATH: &str = "glow-sync/snapshot.json";
const MAX_SYNC_BOOK_BYTES: u64 = 15 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct SyncOutcome {
    pub merged_state: State,
    #[allow(dead_code)]
    pub report: MergeReport,
    #[allow(dead_code)]
    pub books_uploaded: usize,
    #[allow(dead_code)]
    pub books_downloaded: usize,
    pub summary: String,
}

fn device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "GlowDesktop".into())
}

fn http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(12))
        .timeout(Duration::from_secs(45))
        .user_agent("GlowReading/0.2")
        .build()
        .map_err(|_| anyhow!("无法初始化同步网络连接"))
}

fn github_base(settings: &SyncSettings) -> String {
    let trimmed = settings.github_api_base.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        "https://api.github.com".into()
    } else {
        trimmed.to_owned()
    }
}

fn validate_github_settings(settings: &SyncSettings, token: &str) -> Result<(String, String, String)> {
    let owner = settings.github_owner.trim();
    let repo = settings.github_repo.trim();
    let branch = if settings.github_branch.trim().is_empty() {
        "main"
    } else {
        settings.github_branch.trim()
    };
    ensure!(!owner.is_empty(), "请填写 GitHub 用户名（Owner）");
    ensure!(!repo.is_empty(), "请填写 GitHub 同步仓库名（Repo）");
    ensure!(
        !owner.contains('/') && !repo.contains('/'),
        "请分别填写用户名与仓库名，不要包含斜杠"
    );
    ensure!(!token.trim().is_empty(), "请填写 GitHub Personal Access Token");
    Ok((owner.to_owned(), repo.to_owned(), branch.to_owned()))
}

/// Verify GitHub token, repository accessibility, and remind user if repository is public.
pub fn test_github_connection(settings: &SyncSettings, token: &str) -> Result<String> {
    let (owner, repo, branch) = validate_github_settings(settings, token)?;
    let url = format!("{}/repos/{owner}/{repo}", github_base(settings));
    let resp = http_client()?
        .get(&url)
        .bearer_auth(token.trim())
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(|_| anyhow!("无法连接 GitHub API，请检查网络或自定义镜像地址。"))?;

    let status = resp.status().as_u16();
    if status == 401 {
        return Err(anyhow!("GitHub Token 无效或已过期，请重新生成。"));
    }
    if status == 404 {
        return Err(anyhow!(
            "未找到仓库 {owner}/{repo}，或 Token 缺少该仓库的 Contents 读写权限。"
        ));
    }
    ensure!(
        (200..300).contains(&status),
        "GitHub 仓库检查失败（HTTP {status}）。"
    );

    let body: Value = resp.json().unwrap_or(Value::Null);
    let is_private = body["private"].as_bool().unwrap_or(false);
    let can_push = body["permissions"]["push"].as_bool().unwrap_or(true);
    ensure!(
        can_push,
        "当前 Token 只有只读权限，请为仓库 {owner}/{repo} 勾选 Contents: Read and write。"
    );

    if is_private {
        Ok(format!(
            "GitHub 私有仓库 {owner}/{repo}（分支 {branch}）连接成功，可安全开启多端同步！"
        ))
    } else {
        Ok(format!(
            "仓库 {owner}/{repo} 可连接，但当前是公开仓库！强烈建议先到 GitHub 将其改为 Private 私有仓库再同步私人日记。"
        ))
    }
}

pub fn test_sync_connection(settings: &SyncSettings, token: &str) -> Result<String> {
    match settings.mode.as_str() {
        "github" => test_github_connection(settings, token),
        "folder" => {
            let folder = settings.folder_path.trim();
            ensure!(!folder.is_empty(), "请先选择同步文件夹（如坚果云、OneDrive 或 iCloud 目录）");
            let path = Path::new(folder);
            fs::create_dir_all(path).context("无法访问或创建该同步目录")?;
            let probe = path.join(".glow-sync-probe");
            fs::write(&probe, b"ok").context("该同步目录没有写入权限")?;
            let _ = fs::remove_file(&probe);
            Ok(format!("本地同步目录验证通过：{folder}"))
        }
        _ => Err(anyhow!("请先选择一种同步方式（GitHub 私有仓库 或 本地同步盘目录）。")),
    }
}

/// Run a full Pull -> Conflict-Free Merge -> Push sync against either GitHub or a Sync Folder.
pub fn run_sync(
    store_root: &Path,
    local_state: &State,
    settings: &SyncSettings,
    github_token: &str,
) -> Result<SyncOutcome> {
    match settings.mode.as_str() {
        "github" => sync_github(store_root, local_state, settings, github_token),
        "folder" => sync_folder(store_root, local_state, settings),
        _ => Err(anyhow!("当前未开启多端同步，请先在设置中选择同步方式。")),
    }
}

fn github_get_file(
    client: &reqwest::blocking::Client,
    base: &str,
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
    token: &str,
) -> Result<Option<(Vec<u8>, String)>> {
    let url = format!("{base}/repos/{owner}/{repo}/contents/{path}?ref={branch}");
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(|_| anyhow!("连接 GitHub 读取同步快照失败，请检查网络。"))?;

    let status = resp.status().as_u16();
    if status == 404 {
        return Ok(None);
    }
    if status == 401 {
        return Err(anyhow!("GitHub Token 无效或已过期。"));
    }
    ensure!(
        (200..300).contains(&status),
        "从 GitHub 拉取 {path} 失败（HTTP {status}）。"
    );

    let body: Value = resp.json().context("GitHub 返回格式无法解析")?;
    let sha = body["sha"].as_str().unwrap_or("").to_owned();
    let encoded = body["content"]
        .as_str()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .context("GitHub 文件内容解码失败")?;
    Ok(Some((bytes, sha)))
}

fn github_put_file(
    client: &reqwest::blocking::Client,
    base: &str,
    owner: &str,
    repo: &str,
    branch: &str,
    path: &str,
    bytes: &[u8],
    sha: Option<&str>,
    message: &str,
    token: &str,
) -> Result<()> {
    let url = format!("{base}/repos/{owner}/{repo}/contents/{path}");
    let content = base64::engine::general_purpose::STANDARD.encode(bytes);
    let mut payload = json!({
        "message": message,
        "content": content,
        "branch": branch,
    });
    if let Some(sha) = sha.filter(|s| !s.is_empty()) {
        payload["sha"] = json!(sha);
    }
    let resp = client
        .put(&url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&payload)
        .send()
        .map_err(|_| anyhow!("推送同步快照到 GitHub 失败，请检查网络。"))?;

    let status = resp.status().as_u16();
    if status == 409 {
        return Err(anyhow!("云端同步快照刚刚被另一台设备更新，请再点一次同步即可自动合并。"));
    }
    if status == 401 || status == 403 {
        return Err(anyhow!("GitHub 拒绝写入，请确认 Token 具备该仓库的 Contents 写入权限。"));
    }
    ensure!(
        (200..300).contains(&status),
        "写入 GitHub 失败（HTTP {status}）。"
    );
    Ok(())
}

fn sync_github(
    store_root: &Path,
    local_state: &State,
    settings: &SyncSettings,
    token: &str,
) -> Result<SyncOutcome> {
    let (owner, repo, branch) = validate_github_settings(settings, token)?;
    let token = token.trim();
    let base = github_base(settings);
    let client = http_client()?;

    let remote_file =
        github_get_file(&client, &base, &owner, &repo, &branch, SNAPSHOT_PATH, token)?;

    let (mut merged_state, report, remote_sha) = match remote_file {
        Some((bytes, sha)) => {
            let remote_snap: SyncSnapshot =
                serde_json::from_slice(&bytes).context("远端同步快照格式损坏")?;
            let (merged, report) = merge_state_with_snapshot(local_state, &remote_snap);
            (merged, report, Some(sha))
        }
        None => (
            local_state.clone(),
            MergeReport {
                changed_remote: true,
                ..Default::default()
            },
            None,
        ),
    };

    let mut books_uploaded = 0usize;
    let mut books_downloaded = 0usize;

    if settings.sync_books {
        for book in &merged_state.books {
            let local_epub = store_root.join("books").join(&book.id).join("book.epub");
            let remote_path = format!("glow-sync/books/{}.epub", book.id);
            if local_epub.exists() {
                if let Ok(meta) = fs::metadata(&local_epub)
                    && meta.len() <= MAX_SYNC_BOOK_BYTES
                {
                    // Check if remote already has this exact immutable content-addressed book ID
                    let exists = github_get_file(
                        &client,
                        &base,
                        &owner,
                        &repo,
                        &branch,
                        &remote_path,
                        token,
                    )?
                    .is_some();
                    if !exists {
                        let bytes = fs::read(&local_epub)?;
                        github_put_file(
                            &client,
                            &base,
                            &owner,
                            &repo,
                            &branch,
                            &remote_path,
                            &bytes,
                            None,
                            &format!("sync(book): {}", book.title),
                            token,
                        )?;
                        books_uploaded += 1;
                    }
                }
            } else if let Some((bytes, _)) = github_get_file(
                &client,
                &base,
                &owner,
                &repo,
                &branch,
                &remote_path,
                token,
            )? {
                atomic_write(&local_epub, &bytes)?;
                books_downloaded += 1;
            }
        }
    }

    if remote_sha.is_none() || report.changed_remote {
        let next_snapshot = SyncSnapshot::from_state(&merged_state, device_name());
        let bytes = serde_json::to_vec_pretty(&next_snapshot)?;
        let commit_msg = format!(
            "sync: Glow Reading {} ({})",
            now().chars().take(19).collect::<String>(),
            device_name()
        );
        github_put_file(
            &client,
            &base,
            &owner,
            &repo,
            &branch,
            SNAPSHOT_PATH,
            &bytes,
            remote_sha.as_deref(),
            &commit_msg,
            token,
        )?;
    }

    let mut summary = report.summary();
    if books_uploaded > 0 || books_downloaded > 0 {
        summary.push_str(&format!(
            "（电子书上传 {books_uploaded} 本 / 下载 {books_downloaded} 本）"
        ));
    }
    merged_state.sync.last_sync_at = Some(now());
    merged_state.sync.last_sync_summary = Some(summary.clone());

    Ok(SyncOutcome {
        merged_state,
        report,
        books_uploaded,
        books_downloaded,
        summary,
    })
}

fn sync_folder(
    store_root: &Path,
    local_state: &State,
    settings: &SyncSettings,
) -> Result<SyncOutcome> {
    let folder = settings.folder_path.trim();
    ensure!(!folder.is_empty(), "请先选择一个本地同步目录");
    let sync_root = Path::new(folder).join("glow-sync");
    fs::create_dir_all(&sync_root).context("无法创建同步目录")?;
    let snap_path = sync_root.join("snapshot.json");

    let (mut merged_state, report, had_remote) = if snap_path.exists() {
        let bytes = fs::read(&snap_path).context("读取同步目录中的快照失败")?;
        let remote_snap: SyncSnapshot =
            serde_json::from_slice(&bytes).context("同步目录中的快照格式损坏")?;
        let (merged, report) = merge_state_with_snapshot(local_state, &remote_snap);
        (merged, report, true)
    } else {
        (
            local_state.clone(),
            MergeReport {
                changed_remote: true,
                ..Default::default()
            },
            false,
        )
    };

    let mut books_uploaded = 0usize;
    let mut books_downloaded = 0usize;
    if settings.sync_books {
        let books_dir = sync_root.join("books");
        fs::create_dir_all(&books_dir)?;
        for book in &merged_state.books {
            let local_epub = store_root.join("books").join(&book.id).join("book.epub");
            let remote_epub = books_dir.join(format!("{}.epub", book.id));
            if local_epub.exists() && !remote_epub.exists() {
                if let Ok(meta) = fs::metadata(&local_epub)
                    && meta.len() <= MAX_SYNC_BOOK_BYTES
                {
                    let bytes = fs::read(&local_epub)?;
                    atomic_write(&remote_epub, &bytes)?;
                    books_uploaded += 1;
                }
            } else if !local_epub.exists() && remote_epub.exists() {
                let bytes = fs::read(&remote_epub)?;
                atomic_write(&local_epub, &bytes)?;
                books_downloaded += 1;
            }
        }
    }

    if !had_remote || report.changed_remote {
        let next_snapshot = SyncSnapshot::from_state(&merged_state, device_name());
        atomic_write(&snap_path, &serde_json::to_vec_pretty(&next_snapshot)?)?;
    }

    let mut summary = report.summary();
    if books_uploaded > 0 || books_downloaded > 0 {
        summary.push_str(&format!(
            "（电子书同步上传 {books_uploaded} 本 / 拉取 {books_downloaded} 本）"
        ));
    }
    merged_state.sync.last_sync_at = Some(now());
    merged_state.sync.last_sync_summary = Some(summary.clone());

    Ok(SyncOutcome {
        merged_state,
        report,
        books_uploaded,
        books_downloaded,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glow_core::{EchoResult, JournalEntry};
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    #[test]
    fn folder_sync_merges_two_stores_bidirectionally_including_epub() {
        let tmp = tempfile::tempdir().unwrap();
        let sync_dir = tmp.path().join("cloud-drive");
        let dir_a = tmp.path().join("pc-a");
        let dir_b = tmp.path().join("pc-b");

        let mut store_a = glow_core::Store::open(dir_a.clone()).unwrap();
        store_a.install_sample().unwrap();
        store_a
            .record(
                "mood",
                "电脑A写下的手账".into(),
                Some("安静".into()),
                EchoResult {
                    quotes: vec![],
                    reply: "晚安".into(),
                    demo: false,
                },
                None,
            )
            .unwrap();

        let settings = SyncSettings {
            mode: "folder".into(),
            folder_path: sync_dir.to_string_lossy().into_owned(),
            sync_books: true,
            ..Default::default()
        };

        let out_a = run_sync(&dir_a, &store_a.state, &settings, "").unwrap();
        assert_eq!(out_a.books_uploaded, 1);

        let mut store_b = glow_core::Store::open(dir_b.clone()).unwrap();
        store_b.state.journal.push(JournalEntry {
            id: "entry-b".into(),
            created_at: now(),
            kind: "mood".into(),
            input: "电脑B写下的手账".into(),
            mood: Some("期待".into()),
            echo: EchoResult {
                quotes: vec![],
                reply: "早安".into(),
                demo: false,
            },
            location: None,
            favorite: false,
        });

        let out_b = run_sync(&dir_b, &store_b.state, &settings, "").unwrap();
        assert_eq!(out_b.books_downloaded, 1);
        assert_eq!(out_b.merged_state.books.len(), 1);
        assert_eq!(out_b.merged_state.journal.len(), 2);
        let book_id = &out_b.merged_state.books[0].id;
        assert!(dir_b.join("books").join(book_id).join("book.epub").exists());
    }

    #[test]
    fn github_sync_handles_pull_merge_push_over_rest_api() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let server = std::thread::spawn(move || {
            let mut put_received = false;
            for step in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut buf = Vec::new();
                let header_end = loop {
                    let mut b = [0u8; 1];
                    stream.read_exact(&mut b).unwrap();
                    buf.push(b[0]);
                    if buf.ends_with(b"\r\n\r\n") {
                        break buf.len();
                    }
                };
                let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
                let content_len: usize = head
                    .lines()
                    .find_map(|l| {
                        let lower = l.to_ascii_lowercase();
                        lower
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse().ok())
                    })
                    .unwrap_or(0);
                if content_len > 0 {
                    buf.resize(header_end + content_len, 0);
                    stream.read_exact(&mut buf[header_end..]).unwrap();
                }
                if step == 0 {
                    assert!(head.starts_with("GET "));
                    let body = r#"{"message":"Not Found"}"#;
                    write!(
                        stream,
                        "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .unwrap();
                } else {
                    assert!(head.starts_with("PUT "));
                    let payload: Value = serde_json::from_slice(&buf[header_end..]).unwrap();
                    assert!(payload["content"].as_str().is_some());
                    put_received = true;
                    let body = r#"{"content":{"sha":"abc123"}}"#;
                    write!(
                        stream,
                        "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .unwrap();
                }
            }
            put_received
        });

        let tmp = tempfile::tempdir().unwrap();
        let store = glow_core::Store::open(tmp.path().to_path_buf()).unwrap();
        let settings = SyncSettings {
            mode: "github".into(),
            github_owner: "alice".into(),
            github_repo: "glow-sync-private".into(),
            github_branch: "main".into(),
            github_api_base: format!("http://{addr}"),
            sync_books: false,
            ..Default::default()
        };
        let outcome = run_sync(tmp.path(), &store.state, &settings, "ghp_test_token").unwrap();
        assert!(outcome.merged_state.sync.last_sync_at.is_some());
        assert!(server.join().unwrap());
    }
}
