# GitHub 新项目发布指南

适用于 Windows PowerShell。仅发布源码时，完成第 1–4 步后到第 6 步验收仓库首页。有可下载的 EXE、ZIP 或安装包时，再执行第 5 步的打包与发行。

## 1. 验证账号

```powershell
git --version
gh --version
gh api user --jq .login
```

如果找不到 `gh`，运行 `winget install --id GitHub.cli --exact`，然后重新打开 PowerShell。如果尚未登录，执行 `gh auth login --hostname github.com --git-protocol https --web` 并完成网页授权。`gh api user --jq .login` 返回的用户名应与目标账号一致。发布到组织时，还要确认该账号具备创建仓库或推送权限。

## 2. 确定公开范围

- 确认仓库归属、名称、公开或私有，以及可适用的许可证。MIT 适合希望允许使用、修改、分发及商用，并要求保留版权与许可声明的项目。选定后把许可证放在仓库根目录。
- 在 `.gitignore` 中列出本地配置、依赖目录、构建输出和日志。根据项目检查环境文件、凭据、客户资料、内部文档、测试截图和素材授权。已经被 Git 跟踪的文件，后来加入忽略规则仍会继续被跟踪。
- 保留源码、构建配置和依赖锁文件。给需要的环境变量提供不含凭据的 `.env.example`。安装包和校验清单可作为 Releases 附件。

## 3. 补齐可直接照抄的 README

```markdown
# 项目名称

用一两句话说明项目解决什么问题、面向谁、支持哪些平台。

## 下载与安装

提供 Releases 链接、系统要求和运行方式。

## 快速开始

1. 准备输入文件或环境。
2. 完成一次最常见的操作。
3. 说明正常情况下会看到的结果。

## 主要功能

列出最重要的功能，按需加入可公开的截图。

## 从源码运行

列出依赖版本、安装命令、测试命令和构建命令。

## 问题反馈与许可证

提供 Issues 入口，说明提交问题时需要附上的信息，并链接到 LICENSE。
```

## 4. 首次提交和推送

在项目根目录运行。示例假设本地还没有 Git 仓库，且 GitHub 目标仓库也尚未创建。先用 `git rev-parse --show-toplevel` 确认目录。已有本地仓库时跳过初始化，并检查当前分支、`git remote -v` 和已有历史。

```powershell
git init -b main
git status --short --untracked-files=all

# 替换示例源码和配置路径，按实际公开范围逐项添加
git add -- .gitignore README.md LICENSE src

git diff --cached --name-status
git diff --cached --check
git diff --cached

git commit -m "Initial release"
```

确认暂存清单后，再创建仓库。把 `OWNER/REPO` 换成实际归属和名称。私有项目使用 `--private`。

```powershell
gh auth setup-git
gh repo create OWNER/REPO --public --source . --remote origin --push
gh repo view OWNER/REPO --json url,visibility,defaultBranchRef
```

如果目标仓库已经存在，先核对远程地址。若还没有 `origin`，可用 `git remote add origin https://github.com/OWNER/REPO.git` 绑定。远程已存在提交时，先获取并查看历史，再决定如何整合。确认本地和远程历史兼容后运行 `git push -u origin main`。

## 5. 发布可下载版本

1. 新软件默认从 `0.1.0` 开始，Git 标签用 `v0.1.0`。同步应用版本、构建配置、相关锁文件和产物文件名。小修改递增补丁号，例如 `0.1.0` → `0.1.1`。较大功能变更递增次版本并将补丁归零，例如 `0.1.1` → `0.2.0`。主版本按具体项目约定处理。
2. 执行项目的检查、测试和构建，从最终发布目录启动程序，走通最常用的操作。Windows EXE 可用 `(Get-Item $exePath).VersionInfo` 核对文件及产品版本。
3. 计算校验值、编写发布说明并上传。发布说明可包含变更、运行要求、下载文件说明和已知问题。以下示例假设源码已推送、产物文件和发布说明均存在，请替换示例路径和标签。

```powershell
$repo = 'OWNER/REPO'
$version = '0.1.0'
$tag = "v$version"
$exe = Get-Item ".\release\$version\MyApp_${version}_x64.exe"
$checksum = Join-Path $exe.DirectoryName 'SHA256SUMS.txt'
$sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $exe.FullName).Hash.ToLowerInvariant()
"$sha256 *$($exe.Name)" | Set-Content -Encoding ascii -LiteralPath $checksum

# 用实际发布分支的已推送提交 ID 替换占位符
$releaseCommit = 'REPLACE_WITH_PUSHED_COMMIT_ID'
gh release create $tag $exe.FullName $checksum --repo $repo --target $releaseCommit --title $version --notes-file ".\release\$version\Release-notes.md" --latest
```

## 6. 从访客视角验收

- 打开仓库首页，检查 README 的下载入口、许可证和反馈入口。公开项目可以用无痕窗口检查，私有项目用有读权限的账号检查。
- 在 Release 页核对标签、发布状态、文件名及大小。打开 `releases/latest`，确认它指向预期版本。下载最终附件，使用 `Get-FileHash -Algorithm SHA256` 校验，与 `SHA256SUMS.txt` 比对。公开项目也可以通过 GitHub API 查询附件的 `digest`，与本地哈希对照。
- 若推送或上传中断，先用 `git remote -v`、`git ls-remote origin` 或 `gh release view v0.1.0 --repo OWNER/REPO` 确认远程已完成到哪一步，再补做缺失操作。正式发行后需要修改程序时，可以发布修订版本，便于追溯每份安装包。

## 本次发布实例

Unity WebGL 启动器按 `0.1.0` 编号重新打包，同步了 npm、Tauri、Cargo 和对应锁文件的版本。源码和中文说明位于 `main`，Windows x64 EXE 和 SHA-256 清单位于 `v0.1.0`。GitHub 最新下载入口指向 `v0.1.0`，此前的 `v1.0.0` 保留在发行历史中。 [源码仓库](https://github.com/AmenThompson/unity-webgl-launcher) · [发行实例](https://github.com/AmenThompson/unity-webgl-launcher/releases/tag/v0.1.0)

下次可以把这句话连同指南发给助手。

> 请把当前项目发布到 GitHub。目标仓库为 `OWNER/REPO`，可见性为公开或私有，许可证为 `LICENSE_TYPE`。请整理提交范围和 README，运行测试。若有需要打包的程序，请发布指定版本的产物和 SHA-256 清单，并给出仓库和下载链接。
