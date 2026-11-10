# ArceOS 做题指南

**六题共用个人仓库的 `main` 分支。** 换题就是换目录和测试命令，不用切分支，也不用另下用户程序。答案只改 Rust 源码（`.rs`），不用改 Cargo 配置、锁文件、Dockerfile、测试脚本或评测工作流。建议每做通一题就提交一次，后面的题可以接着用前面改好的组件。

## 从第一题开始

1. 领取并克隆[自己的作业仓库](../README.md)，在电脑上启动 Docker。使用 VS Code 时，按 [README 的环境说明](../README.md#2-进入做题环境)在容器中打开仓库，直接进入第 2 步。使用终端时，在仓库根目录运行：

   ```sh
   ./scripts/docker.sh
   ```

   首次会从 Docker Hub 下载课程镜像，之后可离线启动。看到 `root@arceos:/workspace#` 后，就在容器里运行下面的测试；源码仍用电脑上的编辑器修改，保存后容器里立即可见。退出用 `exit`，下次再运行同一条命令。

2. 打开 [第一题的 main.rs](../arceos/exercises/print_with_color/src/main.rs)，在容器里先运行一次：

   ```sh
   ./scripts/test-print.sh
   ```

   观察 QEMU 控制台的问候语。第一题要求给 `Hello, Arceos!` 加上颜色；只打印普通文字还不算通过。改完再运行同一条命令，看到 `print_with_color pass` 就可以提交。

3. 在电脑上的仓库目录提交这次修改：

   ```sh
   git add arceos/exercises/print_with_color/src/main.rs
   git commit -m "完成第一题：彩色输出"
   git push origin main
   ```

   之后每题把 `git add` 的路径换成实际改过的源码。推送后，在个人仓库的 **Actions → 2026a-arceos grading** 查看这次提交的逐题结果。

## 换到下一题

路径均相对仓库根目录。先读练习入口，弄清程序期望什么，再顺着调用去看对应组件。下表的命令可从容器的仓库根目录执行。在 `/workspace/scripts` 下可以直接运行脚本，例如 `./test-print.sh` 或 `./total-test.sh`。

| 题目 | 从哪里读，做什么 | 单题测试 |
| --- | --- | --- |
| 1. 彩色输出 | [问候语](../arceos/exercises/print_with_color/src/main.rs)：让控制台显示带颜色的文字 | `./scripts/test-print.sh` |
| 2. 文件改名 | [测试程序](../arceos/exercises/ramfs_rename/src/main.rs)先写入再改名；看 [RAM 文件系统的目录节点](../arceos/axfs_ramfs/src/dir.rs)，沿 [挂载入口](../arceos/modules/axfs/src/mounts.rs) 确认 `/tmp` 实际使用哪个实现，改名后要能读回内容 | `./scripts/test-ramfs_rename.sh` |
| 3. 内存分配器 | [向量测试](../arceos/exercises/alt_alloc/src/main.rs)会申请大量内存；从 [分配器入口](../arceos/modules/alt_axalloc/) 跟到 [bump_allocator](../arceos/modules/bump_allocator/)，看分配请求怎样得到满足 | `./scripts/test-alt_alloc.sh` |
| 4. HashMap | [集合测试](../arceos/exercises/support_hashmap/src/main.rs)从 `std::collections` 导入 `HashMap`；检查 [axstd](../arceos/ulib/axstd/src/lib.rs) 提供了哪些集合 | `./scripts/test-support_hashmap.sh` |
| 5. 文件映射 | 对照[用户程序](../arceos/payload/mapfile_c/mapfile.c)和 [系统调用](../arceos/exercises/sys_map/src/syscall.rs)，使程序能从映射地址读回文件内容 | `./scripts/test-sys_map.sh` |
| 6. 虚拟化 | 对照[客体程序](../arceos/payload/skernel2/src/main.rs)和 [VM 退出处理](../arceos/exercises/simple_hv/src/main.rs)，使客体能正常关机 | `./scripts/test-simple_hv.sh` |

六题都想检查时，在容器里运行 `./scripts/total-test.sh`。末行是本次总分，`test.output` 列出未通过项。每题 100 分，每次推送会重新测全部六题；只做完一部分时，线上评测标红是正常的。到 **Save measured score and upload to OpenCamp** 看这次分数是否上传成功。
