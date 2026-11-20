use std::{
    collections::VecDeque,
    ffi::OsStr,
    fs::Metadata,
    io,
    path::{Component, Path, PathBuf},
};

use serde::Serialize;
use tokio::fs;

use crate::server::{
    compression::{self, Detection},
    display_root, Format, INDEX,
};

const MAX_DEPTH: usize = 4;
const MAX_DIRS: usize = 1024;
const MAX_ENTRIES: usize = 8192;
const MAX_PROJECTS: usize = 128;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProject {
    name: String,
    root: String,
    entry: String,
    relative_entry: String,
    format: Format,
    fallback: Option<bool>,
    issues: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct CatalogScan {
    root: String,
    projects: Vec<CatalogProject>,
    truncated: bool,
    skipped: usize,
}

#[tauri::command]
pub async fn scan_projects(path: String) -> Result<CatalogScan, String> {
    scan(Path::new(&path)).await
}

async fn directory_root(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("请选择发布总目录".to_owned());
    }
    canonical_directory(path)
        .await
        .map_err(|error| format!("无法读取发布总目录：{error}"))
}

async fn canonical_directory(path: &Path) -> io::Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).await?;
    if !metadata.is_dir() || is_link(&metadata) {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    fs::canonicalize(path).await
}

fn is_link(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

async fn regular_entry(project: &Path) -> io::Result<PathBuf> {
    let path = project.join(INDEX);
    let metadata = fs::symlink_metadata(&path).await?;
    if !metadata.is_file() || is_link(&metadata) {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let entry = fs::canonicalize(path).await?;
    if entry.parent() != Some(project) {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(entry)
}

async fn scan(path: &Path) -> Result<CatalogScan, String> {
    let root = directory_root(path).await?;
    let mut scan = CatalogScan {
        root: display_root(&root),
        projects: Vec::new(),
        truncated: false,
        skipped: 0,
    };
    let mut visited = 1;
    let mut entries = 0;
    let mut queue = VecDeque::from([(root.clone(), 0)]);
    'scan: while let Some((directory, depth)) = queue.pop_front() {
        // A queued directory may have changed on disk while an earlier entry was read.
        if canonical_directory(&directory).await.ok().as_ref() != Some(&directory) {
            scan.skipped += 1;
            continue;
        }
        let mut is_project = false;
        match regular_entry(&directory).await {
            Ok(entry) => {
                let inspection = compression::inspect(&directory).await;
                if inspection.is_unity {
                    if scan.projects.len() == MAX_PROJECTS {
                        scan.truncated = true;
                        break;
                    }
                    is_project = true;
                    let root_name = directory
                        .file_name()
                        .unwrap_or(directory.as_os_str())
                        .to_string_lossy()
                        .into_owned();
                    scan.projects.push(CatalogProject {
                        name: root_name,
                        root: display_root(&directory),
                        entry: display_root(&entry),
                        relative_entry: entry
                            .strip_prefix(&root)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                        format: inspection.detection.format,
                        fallback: inspection.detection.fallback,
                        issues: inspection.issues,
                    });
                }
            }
            Err(error) if error.kind() != io::ErrorKind::NotFound => scan.skipped += 1,
            Err(_) => {}
        }

        let mut children = match fs::read_dir(&directory).await {
            Ok(children) => children,
            Err(error) if depth == 0 => return Err(format!("无法读取发布总目录：{error}")),
            Err(_) => {
                scan.skipped += 1;
                continue;
            }
        };
        let mut next = Vec::new();
        loop {
            let child = match children.next_entry().await {
                Ok(Some(child)) => child,
                Ok(None) => break,
                Err(_) => {
                    scan.skipped += 1;
                    break;
                }
            };
            if entries == MAX_ENTRIES {
                scan.truncated = true;
                break 'scan;
            }
            entries += 1;
            let child_path = child.path();
            let metadata = match fs::symlink_metadata(&child_path).await {
                Ok(metadata) => metadata,
                Err(_) => {
                    scan.skipped += 1;
                    continue;
                }
            };
            if is_link(&metadata) {
                if !child.file_name().eq_ignore_ascii_case(OsStr::new(INDEX)) {
                    scan.skipped += 1;
                }
                continue;
            }
            if !metadata.is_dir() {
                continue;
            }
            if is_project
                && ["Build", "StreamingAssets", "TemplateData"]
                    .iter()
                    .any(|name| child.file_name().eq_ignore_ascii_case(OsStr::new(name)))
            {
                continue;
            }
            if depth == MAX_DEPTH || visited == MAX_DIRS {
                scan.truncated = true;
                continue;
            }
            visited += 1;
            next.push(child_path);
        }
        next.sort_unstable();
        queue.extend(next.into_iter().map(|child| (child, depth + 1)));
    }
    scan.projects
        .sort_unstable_by(|left, right| left.relative_entry.cmp(&right.relative_entry));
    Ok(scan)
}

pub(crate) async fn preflight(
    entry: &Path,
    selected_root: &Path,
) -> Result<(PathBuf, Detection), String> {
    let root = directory_root(selected_root).await?;
    let scope = PathBuf::from(display_root(&root));
    // Accept paths from either the original picker spelling or the normalized API result.
    let relative = entry
        .strip_prefix(&scope)
        .or_else(|_| entry.strip_prefix(&root))
        .or_else(|_| entry.strip_prefix(selected_root))
        .map_err(|_| "所选项目不在发布总目录内".to_owned())?;
    let Some(parent) = relative.parent() else {
        return Err("请选择目录扫描返回的 index.html".to_owned());
    };
    if !relative
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case(OsStr::new(INDEX)))
    {
        return Err("请选择目录扫描返回的 index.html".to_owned());
    }
    let mut project = root;
    for (depth, component) in parent.components().enumerate() {
        let Component::Normal(component) = component else {
            return Err("所选项目路径无效".to_owned());
        };
        if depth == MAX_DEPTH {
            return Err("所选项目超出目录扫描深度".to_owned());
        }
        project.push(component);
        let canonical = canonical_directory(&project)
            .await
            .map_err(|_| "所选项目目录无法读取或包含链接".to_owned())?;
        if canonical != project {
            return Err("所选项目目录已变更，请重新扫描".to_owned());
        }
    }
    regular_entry(&project)
        .await
        .map_err(|_| "所选项目入口已不可用，请重新扫描".to_owned())?;
    let inspection = compression::inspect(&project).await;
    if !inspection.is_unity {
        return Err("所选入口不是可识别的 Unity WebGL 项目".to_owned());
    }
    if !inspection.issues.is_empty() {
        return Err(inspection.issues.join("；"));
    }
    Ok((project, inspection.detection))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("unity-catalog-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(std::fs::canonicalize(path).unwrap())
        }

        fn release(&self, relative: &str, missing: bool) -> PathBuf {
            let root = self.0.join(relative);
            std::fs::create_dir_all(root.join("Build")).unwrap();
            std::fs::write(
                root.join(INDEX),
                "<script>const buildUrl = 'Build'; const loaderUrl = buildUrl + '/game.loader.js'; createUnityInstance(canvas, { dataUrl: buildUrl + '/game.data.gz', frameworkUrl: buildUrl + '/game.framework.js.gz', codeUrl: buildUrl + '/game.wasm.gz' });</script>",
            ).unwrap();
            for file in [
                "game.loader.js",
                "game.data.gz",
                "game.framework.js.gz",
                "game.wasm.gz",
            ] {
                if !missing || file != "game.wasm.gz" {
                    std::fs::write(root.join("Build").join(file), b"resource").unwrap();
                }
            }
            root
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test]
    async fn catalog_scans_root_and_four_levels_with_sorted_relative_paths() {
        let workspace = TempTree::new();
        workspace.release("", false);
        workspace.release("课程/嵌套/最终/发布", false);
        workspace.release("课程/嵌套/最终/发布/过深", false);
        std::fs::create_dir_all(workspace.0.join("site")).unwrap();
        std::fs::write(
            workspace.0.join("site/index.html"),
            "<h1>ordinary website</h1>",
        )
        .unwrap();
        let catalog = scan(&workspace.0).await.unwrap();
        assert!(catalog.truncated);
        assert_eq!(catalog.skipped, 0);
        assert_eq!(catalog.projects.len(), 2);
        assert_eq!(catalog.projects[0].relative_entry, "index.html");
        assert_eq!(
            catalog.projects[1].relative_entry,
            "课程/嵌套/最终/发布/index.html"
        );
        assert_eq!(catalog.projects[1].format, Format::Gzip);
        assert_eq!(catalog.projects[1].fallback, Some(false));
        assert!(catalog.projects[1].issues.is_empty());
        assert!(preflight(
            Path::new(&catalog.projects[1].entry),
            Path::new(&catalog.root)
        )
        .await
        .is_ok());
    }

    #[cfg(any(windows, unix))]
    #[tokio::test]
    async fn catalog_ignores_linked_project_aliases() {
        let workspace = TempTree::new();
        let target = workspace.release("original", false);
        let alias = workspace.0.join("alias");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &alias).unwrap();
        #[cfg(windows)]
        {
            let junction = std::process::Command::new("cmd.exe")
                .args(["/C", "mklink", "/J"])
                .arg(display_root(&alias))
                .arg(display_root(&target))
                .output()
                .unwrap();
            assert!(junction.status.success());
        }
        let catalog = scan(&workspace.0).await.unwrap();
        assert_eq!(catalog.projects.len(), 1);
        assert_eq!(catalog.projects[0].name, "original");
        assert_eq!(catalog.skipped, 1);
        assert!(preflight(&alias.join(INDEX), &workspace.0).await.is_err());
        #[cfg(windows)]
        std::fs::remove_dir(&alias).unwrap();
    }

    #[tokio::test]
    async fn resolves_custom_build_directory_and_ignores_stale_build_files() {
        let workspace = TempTree::new();
        let project = workspace.release("custom", false);
        std::fs::write(
            project.join(INDEX),
            "<script>const buildUrl = 'Nested/Output'; createUnityInstance(canvas, {codeUrl: buildUrl + '/game.wasm.gz'});</script>",
        ).unwrap();
        let initial = scan(&workspace.0).await.unwrap();
        assert_eq!(initial.projects.len(), 1);
        assert!(initial.projects[0]
            .issues
            .iter()
            .any(|issue| issue.contains("Output") && issue.contains("game.wasm.gz")));
        std::fs::create_dir_all(project.join("Nested/Output")).unwrap();
        std::fs::write(project.join("Nested/Output/game.wasm.gz"), b"compressed").unwrap();
        let refreshed = scan(&workspace.0).await.unwrap();
        assert!(refreshed.projects[0].issues.is_empty());
        assert_eq!(refreshed.projects[0].format, Format::Gzip);
    }

    #[tokio::test]
    async fn reports_missing_references_and_revalidates_catalog_scope() {
        let workspace = TempTree::new();
        let in_catalog = workspace.release("演示/项目", true);
        let outside = TempTree::new();
        let valid_elsewhere = outside.release("其他", false);
        let catalog = scan(&workspace.0).await.unwrap();
        assert_eq!(catalog.projects.len(), 1);
        assert!(catalog.projects[0]
            .issues
            .iter()
            .any(|s| s.contains("game.wasm.gz")));
        assert!(preflight(&in_catalog.join(INDEX), &workspace.0)
            .await
            .unwrap_err()
            .contains("game.wasm.gz"));
        assert!(preflight(&valid_elsewhere.join(INDEX), &workspace.0)
            .await
            .unwrap_err()
            .contains("不在发布总目录"));
        assert!(scan(&in_catalog.join(INDEX)).await.is_err());
    }
}
