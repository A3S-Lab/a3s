# libkrun 改动推送总结

## 推送时间
2026-03-12 21:00

## 推送状态
✅ **成功推送到 GitHub**

## 仓库信息
- **仓库**: git@github.com:A3S-Lab/libkrun.git
- **分支**: main
- **状态**: 与远程同步

## 推送的提交

### 1. feat(virtiofs): add no_fsync field to FsDeviceConfig initializers
**提交 ID**: e4d7c11
**日期**: 2026-03-12 18:42:35
**作者**: RoyLin

**改动**：
- 文件：`src/libkrun/src/lib.rs`
- 在 `krun_set_root`, `krun_add_virtiofs`, `krun_add_virtiofs2` 函数中添加 `no_fsync: false` 字段

**说明**：
完成 no_fsync 实现，确保所有创建 FsDeviceConfig 结构体的代码路径都包含新字段。

---

### 2. feat(virtiofs): add no_fsync option for macOS performance
**提交 ID**: 062142c
**日期**: 2026-03-12 16:57:30
**作者**: RoyLin

**改动**：
- `src/devices/src/virtio/fs/device.rs` - 添加 no_fsync 支持
- `src/devices/src/virtio/fs/macos/passthrough.rs` - 实现 fsync 跳过逻辑
- `src/vmm/src/builder.rs` - 添加配置传递
- `src/vmm/src/vmm_config/fs.rs` - 添加配置字段

**功能**：
添加 `LIBKRUN_NO_FSYNC` 环境变量支持，允许禁用 VirtioFS 的 fsync 操作以提升 macOS 性能。

**使用方式**：
```bash
export LIBKRUN_NO_FSYNC=1
```

**警告**：
禁用 fsync 会降低数据持久性保证。仅在开发/测试环境中使用。

**目的**：
修复 OpenClaw 在 macOS 上的 a3s-box 启动挂起问题，避免慢速 fsync 操作。

---

### 3. fix(tsi): change EventSet::OUT error to debug log for residual events
**提交 ID**: e8b1854
**作者**: RoyLin

**改动**：
将 TSI 中残留事件的错误日志改为调试日志，减少不必要的错误输出。

---

## 推送范围

**从**: 6e49118 (chore: bump version to 1.17.5)
**到**: e4d7c11 (feat(virtiofs): add no_fsync field to FsDeviceConfig initializers)

**提交数量**: 3 个

---

## 功能总结

### no_fsync 功能（完整实现）

这是一个针对 macOS 性能优化的重要功能：

**问题背景**：
- macOS APFS 文件系统的 fsync 操作非常慢
- Node.js 的 `writeFileSync()` 默认调用 fsync
- 导致 OpenClaw 等应用在 a3s-box 中启动挂起

**解决方案**：
- 添加 `no_fsync` 配置选项
- 通过 `LIBKRUN_NO_FSYNC=1` 环境变量控制
- 当启用时，fsync 操作立即返回，不执行实际的磁盘同步

**实现细节**：
1. 在 PassthroughFs Config 中添加 `no_fsync` 字段
2. 修改 `fsync()` 方法检查标志并提前返回
3. 在 libkrun 初始化时读取环境变量
4. 在所有 FsDeviceConfig 初始化路径中添加字段

**影响范围**：
- 仅影响 macOS 平台
- 仅在显式设置环境变量时启用
- 不影响其他平台或默认行为

---

## 验证

### Git 状态
```bash
$ cd /Users/roylin/Desktop/code/libkrun
$ git status
On branch main
Your branch is up to date with 'origin/main'.
nothing to commit, working tree clean

$ git log --oneline -3 --decorate
e4d7c11 (HEAD -> main, origin/main, origin/HEAD) feat(virtiofs): add no_fsync field to FsDeviceConfig initializers
062142c feat(virtiofs): add no_fsync option for macOS performance
e8b1854 fix(tsi): change EventSet::OUT error to debug log for residual events
```

### 远程同步
- ✅ 本地分支与远程同步
- ✅ 所有提交已推送
- ✅ 无待推送的改动

---

## 下游影响

### a3s-box
a3s-box 已经更新到 libkrun-sys 0.1.5，包含了这些改动：
- ✅ 已在 a3s-box v0.8.8 中集成
- ✅ 已推送到远程
- ✅ 已触发发布流程

### 使用方式
用户现在可以通过以下方式使用 no_fsync 功能：

```bash
# 方式 1: 直接设置环境变量
export LIBKRUN_NO_FSYNC=1
a3s-box run ...

# 方式 2: 在命令前设置
LIBKRUN_NO_FSYNC=1 a3s-box run ...
```

---

## 相关文档

本次调查和实现过程中创建的文档：
- `NGINX_VS_OPENCLAW_COMPARISON.md` - nginx 与 OpenClaw 对比测试
- `OPENCLAW_HIGH_CPU_ANALYSIS.md` - OpenClaw 高 CPU 使用率分析
- `OPENCLAW_INVESTIGATION_SUMMARY.md` - 完整调查总结
- `MACOS_GIC_SOLUTIONS_RESEARCH.md` - macOS GIC 锁竞争解决方案研究
- `A3S_BOX_COMPATIBILITY_TEST.md` - a3s-box 兼容性测试报告
- `OPENCLAW_SIMULATOR_TEST.md` - OpenClaw 模拟器测试报告
- `RECENT_CHANGES_SUMMARY.md` - 最近改动总结
- `A3S_BOX_V0.8.8_RELEASE.md` - a3s-box v0.8.8 发布报告

---

## 相关链接

- **GitHub 仓库**: https://github.com/A3S-Lab/libkrun
- **提交历史**: https://github.com/A3S-Lab/libkrun/commits/main
- **最新提交**: https://github.com/A3S-Lab/libkrun/commit/e4d7c11

---

## 后续工作

### 已完成 ✅
1. ✅ 实现 no_fsync 功能
2. ✅ 推送 libkrun 改动
3. ✅ 更新 a3s-box 到 libkrun-sys 0.1.5
4. ✅ 发布 a3s-box v0.8.8

### 可选的后续工作
1. 监控用户反馈
2. 收集性能数据
3. 考虑是否需要文档更新
4. 评估是否需要向上游 libkrun 项目提交 PR

---

**推送完成！** 🎉

libkrun 的所有改动已成功推送到 GitHub，并已集成到 a3s-box v0.8.8 中。
