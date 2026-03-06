# DevPilot

本地开发服务器管理工具。自动发现你电脑上正在运行的所有开发项目，一键打开、启动、停止，不需要切换到编程工具。

**专为使用 AI 编程工具（Claude Code、Cursor、Codex 等）的用户设计** — 当你同时用多个工具开发多个项目时，DevPilot 帮你看清全局。

## 功能

- **自动发现** — 自动扫描本机所有运行中的开发服务器（Next.js、Vite、Django、Express 等）
- **一键打开** — 知道每个项目跑在哪个端口，点一下就在浏览器里打开
- **启动/停止** — 不需要打开编程工具，直接管理项目的运行状态
- **前后端分组** — 同一个项目的前端和后端服务自动合并显示
- **项目记忆** — 跑过的项目自动记住，下次可以直接启动
- **智能检测** — 自动识别项目类型、启动命令、包管理器（pnpm/yarn/bun）

## 截图

<!-- 在这里添加应用截图 -->

## 安装

从 [Releases](../../releases) 页面下载最新的安装包：

- **macOS**: `DevPilot_x.x.x_aarch64.dmg` (Apple Silicon) 约 4MB

> Windows/Linux 版本计划中

## 从源码构建

需要：Node.js 18+、Rust 1.70+、pnpm

```bash
# 安装依赖
pnpm install

# 开发模式
pnpm tauri dev

# 构建安装包
pnpm tauri build
```

## 工作原理

DevPilot 不是运行时引擎，而是一个驾驶舱：

1. 通过 `lsof` 扫描本机 3000-9999 端口范围内的监听进程
2. 识别 node/python/go 等开发服务器进程
3. 读取项目目录中的 package.json/go.mod 等，自动检测项目类型
4. 同一项目的前端/后端服务按目录结构自动分组

## 技术栈

- **前端**: React + TypeScript
- **后端**: Rust (Tauri v2)
- **打包**: ~4MB DMG

## License

[MIT](LICENSE)
