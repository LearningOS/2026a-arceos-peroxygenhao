# 项目先导阶段：用 ArceOS 搭建组件化操作系统

在六项 RISC-V 练习中，你将从控制台输出开始，逐步接触文件系统、内存分配、系统调用和虚拟化。**六题都在自己的作业仓库 `main` 分支完成，每题 100 分，满分 600 分。**

## 1. 领取自己的作业仓库

1. 加入 [OpenCamp 秋冬季训练营](https://opencamp.cn/os2edu/camp/2026fall)，绑定用于提交作业的 GitHub 账号。
2. 用同一账号打开 [ArceOS 领取表单](https://github.com/LearningOS/2026a-enroll/issues/new?template=arceos.yml)，点击 **Create**。在机器人回复中接受邀请，再打开分配给你的仓库。
3. 把下面的 `YOUR_GITHUB_LOGIN` 换成自己的 GitHub 登录名，克隆个人作业仓库，不要克隆公共模板交作业。

```sh
git clone https://github.com/LearningOS/2026a-arceos-YOUR_GITHUB_LOGIN.git
cd 2026a-arceos-YOUR_GITHUB_LOGIN
```

内核、练习和测试脚本都在这个仓库中。不需要像 rCore 那样切换章节分支或另外克隆 `user/`。如果能看到仓库却不能推送，先确认当前登录账号是否已接受邀请；找不到有效邀请时，把原申请的 Issue 链接交给助教。

## 2. 进入做题环境

安装并启动 Docker。用 VS Code 做题时，先安装 **Dev Containers** 扩展，打开作业仓库根目录，选择 **Dev Containers: Reopen in Container**。首次打开会下载课程镜像并自动安装 rust-analyzer 扩展。等索引完成，按住 Ctrl（macOS 用 ⌘）点击第一题的 `println!` 即可跳到定义，也可以按 F12。直接在 VS Code 的容器终端运行测试。

只用终端做题时，在仓库根目录执行：

```sh
./scripts/docker.sh
```

首次会从 Docker Hub 下载课程镜像，以后启动时复用本地镜像。镜像内置与本期评测对应的 Rust `nightly-2024-09-04`、RISC-V musl GCC `11.2.1` 和 QEMU `8.2.2`，不用在电脑上分别安装。Apple 芯片的 Mac 也使用同一套 x86_64 工具。Windows 请在 WSL2 中执行。

在电脑上编辑源码，在容器里运行 `./scripts/test-print.sh` 测第一题。六题都在 `main` 完成，见 **[做题指南：每题改哪里、怎么测、怎么提交](docs/STUDENT_GUIDE.md)**。

## 3. 六项练习

配合 [ArceOS 教材](https://rcore-os.cn/arceos-tutorial-book/) 和[本期课程资料](https://opencamp.cn/os2edu/camp/2026fall/stage/6) 学习。下表链接是各题的代码入口；有些题需要沿调用关系修改内核组件，而不只是改入口程序。

| 练习入口 | 要实现或验证什么 | 本地测试 |
| --- | --- | --- |
| [print_with_color](arceos/exercises/print_with_color/) | 让控制台问候语带颜色输出 | `./scripts/test-print.sh` |
| [ramfs_rename](arceos/exercises/ramfs_rename/) | 支持内存文件系统中同一目录下的文件重命名 | `./scripts/test-ramfs_rename.sh` |
| [alt_alloc](arceos/exercises/alt_alloc/) | 让练习使用替代分配器，完成向量分配和排序测试 | `./scripts/test-alt_alloc.sh` |
| [support_hashmap](arceos/exercises/support_hashmap/) | 在内核应用中支持 `HashMap` 并通过集合测试 | `./scripts/test-support_hashmap.sh` |
| [sys_map](arceos/exercises/sys_map/) | 支持用户程序映射文件，读回文件内容 | `./scripts/test-sys_map.sh` |
| [simple_hv](arceos/exercises/simple_hv/) | 运行客体内核，处理其退出并验证正常关机 | `./scripts/test-simple_hv.sh` |

一次检查六题，运行 `./scripts/total-test.sh`。最后一行是总分，`test.output` 列出失败项；不要只看脚本退出状态。[课件](course/) 可配合阅读，[往期挑战](challenges/) 不在这六题的计分范围内。

## 4. 提交与看分

将实际修改的源码加入 Git 并提交，再执行 `git push origin main`。不要提交生成的镜像或测试日志。

打开自己仓库的 **Actions**，选择这次提交，查看 **Test exercises and calculate score** 的逐题结果。每次都重新计算六题，例如这次通过四题就是 **400/600**，不是按提交次数累加，也不保留历史最高分。

有题未过时测试作业会标红；如果六题均已测完，仍可上传本次部分分数。由本人账号提交后，看 **Save measured score and upload to OpenCamp** 是否成功，再到 OpenCamp 核对。若环境准备失败、没有完整结果或上传报错，把这次 Actions 运行链接交给助教。
