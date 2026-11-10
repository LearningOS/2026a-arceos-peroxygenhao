# 项目先导阶段：ArceOS 六项练习实验报告

六题都在个人仓库 `2026a-arceos-peroxygenhao` 的 `main` 分支完成，只改动 Rust 源码（另有一处 `[patch.crates-io]`，见第二题），
没有改测试脚本、评测工作流。

| 练习 | 改动的文件 | 本地测试脚本 |
| --- | --- | --- |
| 1. `print_with_color` | `arceos/exercises/print_with_color/src/main.rs` | `./scripts/test-print.sh` |
| 2. `ramfs_rename` | `arceos/axfs_ramfs/src/dir.rs`、`arceos/Cargo.toml`、`arceos/Cargo.lock` | `./scripts/test-ramfs_rename.sh` |
| 3. `alt_alloc` | `arceos/modules/bump_allocator/src/lib.rs` | `./scripts/test-alt_alloc.sh` |
| 4. `support_hashmap` | `arceos/ulib/axstd/src/lib.rs`、`arceos/ulib/axstd/src/collections/mod.rs` | `./scripts/test-support_hashmap.sh` |
| 5. `sys_map` | `arceos/exercises/sys_map/src/syscall.rs` | `./scripts/test-sys_map.sh` |
| 6. `simple_hv` | `arceos/exercises/simple_hv/src/main.rs` | `./scripts/test-simple_hv.sh` |

## 实验环境

课程官方环境是 Docker 镜像（Ubuntu 24.04 + QEMU 8.2 + `nightly-2024-09-04` + riscv64 musl GCC）。
本机没有 Docker / WSL，于是按镜像里的组件在本机复现了一套等价环境，跑的就是仓库自带的测试脚本：

| 组件 | 说明 |
| --- | --- |
| 工具链 | `rustup` 安装 `nightly-2024-09-04`，目标 `riscv64gc-unknown-none-elf`，含 `llvm-tools`（`rust-objcopy`） |
| 模拟器 | QEMU 8.2（Windows 版 `qemu-system-riscv64`，自带 OpenSBI 1.4 固件） |
| 构建工具 | GNU Make（w64devkit）+ Git Bash 提供 POSIX 工具 |
| 交叉编译器 | `zig cc -target riscv64-linux-musl` 充当 `riscv64-linux-musl-gcc`，用于编译 C 测例 `mapfile` |
| 磁盘镜像 | 用 Python 补齐 `mkfs.fat` / `mmd` / `mcopy`，生成与 CI 一致的分区布局（FAT32、512B 扇区） |
| bindgen | `arceos_posix_api` 的 build script 需要 `libclang`，使用 libclang 18 |

另外因为仓库路径含中文会破坏 QEMU 打开镜像文件，构建与测试统一在 `subst` 出来的 ASCII 盘符下进行。

## 一、print_with_color：让问候语带颜色

控制台没有任何“颜色”接口，ANSI SGR 转义序列由串口原样送到终端，所以最小改动是在应用层把问候语包进 `ESC[1;32m … ESC[0m`：

```rust
const COLOR_START: &str = "\x1b[1;32m";
const COLOR_END: &str = "\x1b[0m";
println!("{}[WithColor]: Hello, Arceos!{}", COLOR_START, COLOR_END);
```

同理也可以改 `axstd::io` 或 `axhal` 的控制台写入路径，但那会把启动日志一起染色，影响范围远大于这一行问候语，
因此只在打印这一层着色。测试脚本先检查最后 20 行里有 `ESC[`，再剥掉颜色码确认文字仍然是 `Hello, Arceos!`。

## 二、ramfs_rename：为 RamFS 补上 rename

`make run A=exercises/ramfs_rename/ BLK=y` 复现的报错来自 `axfs_vfs::VfsNodeOps::rename` 的默认实现
（返回 `Unsupported`）：`axfs_ramfs` 的目录节点没有实现重命名。

两个问题要解决：

1. **让工作区真正用上仓库里的 `axfs_ramfs`。** `modules/axfs` 与练习 crate 依赖的是 crates.io 上的
   `axfs_ramfs 0.1`，不 patch 的话改本地组件没有任何效果。按课件提示加 `[patch.crates-io]`：

   ```toml
   [patch.crates-io]
   axfs_ramfs = { path = "./axfs_ramfs" }
   ```

   随后 `cargo update -p axfs_ramfs` 让 `Cargo.lock` 指向本地 0.1.1（`cargo fetch --locked` 仍然通过）。

2. **实现 rename。** 在 `axfs_ramfs/src/dir.rs` 里新增：

   * `lookup_dir()`：按路径从当前节点向下找到目标目录，支持 `..`；
   * `rename_node()`：在同一目录内把 `src` 的节点改名成 `dst`（目标已存在返回 `AlreadyExists`）；
   * `VfsNodeOps::rename()`：解析 `src`/`dst` 的父路径与末段名字，校验不是 `.`/`..`，
     并把“目标父目录”限制为源目录本身——即只支持改名、不支持移动。
     调用方 `axfs::root::rename` 传下来的目标路径可能带挂载点前缀（`/tmp/f2`），
     解析不到时按“同目录改名”处理，这正是本练习的语义。

运行结果：

```
Create '/tmp/f1' and write [hello] ...
Rename '/tmp/f1' to '/tmp/f2' ...
Read '/tmp/f2' content: [hello] ok!
[Ramfs-Rename]: ok!
```

## 三、alt_alloc：实现 bump 分配算法

练习只允许改 `modules/bump_allocator`。`EarlyAllocator` 的 12 个方法原本都是 `todo!()`，这里按
“一段内存、两端生长”的经典 bump 设计实现：

```
[ 已分配字节区 | 空闲区 | 已分配页区 ]
start        b_pos    p_pos        end
```

* `init()`：上界按页对齐（页从尾部向前分配），下界留给字节分配按需对齐；
* `alloc()`：`b_pos` 向上对齐到 `layout.align()` 后切出 `size`，跨过 `p_pos` 即 `NoMemory`；
* `dealloc()`：用 `count` 统计存活分配，归零时整体回收字节区（bump 指针只能回退）；
* `alloc_pages()`：从 `p_pos` 向下对齐（`align_pow2` 是**字节**对齐，需为 2 的幂且不小于页大小），
  只要不越过字节区即可；`dealloc_pages()` 只在释放的是最后一段时回退；
* `total_bytes/used_bytes/available_bytes` 与对应的 `*_pages` 按上述布局计算；
* `add_memory()`：只有与当前区间相邻时才合并，否则返回 `MemoryOverlap`（bump 分配器无法管理碎片区间）。

测例连推 300 万个 `u32`（约 12 MB）并排序，运行结果 `Bump tests run OK!`。

## 四、support_hashmap：让 axstd 提供 HashMap

`no_std` 的 `alloc::collections` 里没有 `HashMap`（它依赖 `std` 的随机哈希种子），`axstd` 只把
`alloc::collections` 转发出来，所以练习里的 `use std::collections::HashMap` 编译不过，而且不能新增依赖
（CI 只跑 `cargo fetch --locked`）。

于是给 `axstd` 加一个自己的 `collections` 模块（`ulib/axstd/src/collections/mod.rs`），
转发 `alloc` 的树形容器，并实现一个线性探测的开放寻址哈希表：

* `DefaultHasher`：FNV-1a 64 打底，末尾再做一次 splitmix64 混合，让低位（用作槽位下标）分布均匀；
* `RandomState`：等价于 `std` 的随机种子构造器，用全局计数器与栈地址混合出每个 map 独立的种子；
* `HashMap<K, V, S = RandomState>`：`new/with_capacity/with_hasher/insert/get/get_mut/remove/
  contains_key/len/clear/iter/iter_mut/keys/values/reserve`，以及 `Debug/Default/Extend/
  FromIterator/PartialEq/Clone/IntoIterator`；
* 删除用墓碑标记（`Slot::Deleted`）保持探测链完整，装载因子 3/4 时扩容并顺带清理墓碑。

`axstd::lib.rs` 里把原来的 `pub use alloc::{… collections …}` 换成 `pub mod collections;`。
插入 5 万条 `format!("key_{value}")` 再全量遍历校验，输出 `Memory tests run OK!`。

## 五、sys_map：实现 sys_mmap

`exercises/sys_map` 是一个“迷你宏内核”：`loader` 把 musl 静态链接的 `mapfile` 装进用户地址空间，
`syscall.rs` 里 `sys_mmap` 原本是 `unimplemented!()`。

实现要点：

1. 长度按 4 KiB 向上取整；`addr == NULL`（或未置 `MAP_FIXED`）时在用户地址空间里用
   `AddrSpace::find_free_area()` 找空档，提示地址取 `0x1000_0000`（在应用镜像与用户栈之间）；
2. `map_alloc(vaddr, size, flags, populate = true)` 建立映射；`MmapProt → MappingFlags` 的转换脚手架里已经给好；
3. 非 `MAP_ANONYMOUS` 时从 `fd` 读出文件内容（必要时先 `lseek` 到 `offset`），再用 `AddrSpace::write()`
   拷贝进新页；匿名映射本身就是零页，不用处理；
4. 失败路径把错误码按 Linux 约定取负返回（`EINVAL/ENOMEM/EBADF/EFAULT`），并回收已建立的映射。

测例执行到 `mmap(NULL, 32, PROT_READ, MAP_PRIVATE, fd, 0)` 后打印：

```
sys_mmap: addr=0x0, length=0x20, prot=0x1, flags=0x2, fd=3, offset=0x0
sys_mmap: mapped 15 bytes from fd 3 at 0x10000000 (1 page(s))
Read back content: hello, arceos!
MapFile ok!
```

## 六、simple_hv：处理 VM-Exit，让客体正常关机

客体 `skernel2` 在 VS-mode 依次执行：

```
80200000: csrr a1, mhartid      # 机器级 CSR，VS-mode 没有 → IllegalInstruction
80200004: ld   a0, 0x40(zero)   # 客体物理地址未映射 → LoadGuestPageFault
80200008: li   a7, 8            # SBI legacy shutdown
8020000a: ecall                 # → VirtualSupervisorEnvCall，交回 Hypervisor
```

`vmexit_handler` 原来对前两种情况直接 `panic!`，这里补上两条“指令级模拟”：

* `IllegalInstruction`：客体没有 M 模式，读 `mhartid` 由 Hypervisor 代答，写 `a1 = 0x1234`；
* `LoadGuestPageFault`：客体读的 `0x40` 当作一块虚拟设备的寄存器，写 `a0 = 0x6688`；
* 两者都要把 `sepc` 跳过**这条指令的长度**，否则 `sret` 回去会重复触发同一个异常（死循环）。
  客体镜像是装进新分配的页帧里的，客体物理地址不等于 Hypervisor 的物理地址，无法用线性映射直接读指令，
  因此按 `payload/skernel2` 的反汇编固化长度：两个都是 4 字节（`csrr a1, mhartid` 是 `f3 25 40 f1`）；
* 收到 SBI `Reset(Shutdown)` 后打印 `Shutdown vm normally!`，随后由 `axhal::misc::terminate()`
  正常关机，而不是 panic——这样 QEMU 能以 0 退出，测试脚本才认为“正常关机”。

运行结果：

```
VmExit Reason: IllegalInstruction at 0x80200000, emulate csrr mhartid
VmExit Reason: LoadGuestPageFault at 0x80200004, emulate device register 0x40
VmExit Reason: VSuperEcall: Some(Reset(Reset { reset_type: Shutdown, reason: NoReason }))
a0 = 0x6688, a1 = 0x1234
Shutdown vm normally!
Hypervisor exit [ok]!
```

## 七、验证

在本机用仓库自带脚本逐题验证（`./scripts/test-*.sh`，等价于 CI 的
`make A=exercises/<name>/ build` + `make A=exercises/<name>/ justrun` 加输出断言）：

```
print_with_color pass
ramfs_rename pass
alt_alloc pass
support_hashmap pass
sys_map pass
simple_hv pass
```

`./scripts/total-test.sh` 的最后一行为总分 `600`。
