# modbus-manager

Modbus TCP 管理服务,为流水线/输送线提供基于 HTTP 的 Modbus 读写接口,并内置 Scalar API 文档。

## 功能

- 健康检查: `GET /health`
- 流水线写入: `POST /api/conveyor/write`
- 发送成功确认读取: `GET /api/conveyor/write-success`
- 可放货位读取: `GET /api/conveyor/can-putdown`
- 需放货通知写入: `POST /api/conveyor/need-putdown`
- Scalar API 文档: `GET /scalar`（数据源: `/openapi.json`）
- OpenTelemetry 链路跟踪与结构化日志

## 快速开始

### 本地运行

```bash
cargo build --release
./target/release/modbus-manager
```

服务默认监听 `0.0.0.0:3000`,配置文件为项目根目录的 `config.toml`。

### 配置

主要配置项:

- `[server]` — 监听地址
- `[logging]` — 日志保留天数与清理间隔
- `[modbus]` — Modbus 连接超时、重连策略、连接池大小
- `[conveyor]` — 流水线路由、读取检查、Webhook 等

首次安装时脚本会自动复制 `config.toml`；升级时不会覆盖已有配置。

## 安装脚本

Linux 服务器一键安装/升级(默认使用 gnu/glibc 动态链接包):

```bash
curl -fsSL https://raw.githubusercontent.com/zy97/modbus-manager/main/scripts/install-modbus-manager.sh | sudo bash
```

指定版本:

```bash
curl -fsSL .../install-modbus-manager.sh | sudo bash -s -- 1.0.0
```

产物选择:

- 默认: `modbus-manager-x86_64-unknown-linux-gnu.tar.xz`
- 强制 musl 静态链接包: `FORCE_MUSL=1 curl -fsSL ... | sudo -E bash`
- 强制 gnu/glibc 动态链接包: `FORCE_GNU=1 curl -fsSL ... | sudo -E bash`
- 直接指定资产文件名: `TARGET=modbus-manager-x86_64-unknown-linux-musl.tar.xz curl -fsSL ... | sudo -E bash`

国内访问 GitHub Release 较慢时,可通过 `GH_PROXY` 设置代理前缀(末尾带 `/`):

```bash
GH_PROXY="https://ghfast.top/" curl -fsSL .../install-modbus-manager.sh | sudo -E bash
```

## Scalar API 文档

启动服务后,打开:

```
http://<服务器IP>:3000/scalar
```

Scalar 会从 `/openapi.json` 加载接口定义并渲染交互式文档。

## 构建发布产物

本项目使用 [cargo-dist](https://github.com/axodotdev/cargo-dist) 管理 Release 产物。

```bash
dist plan
dist build
```

当前支持的目标平台:

- `x86_64-unknown-linux-gnu`
- `x86_64-unknown-linux-musl`
- `x86_64-pc-windows-msvc`

## 目录结构

```
.
├── config.toml                 # 默认配置文件
├── modbus-manager/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── config/             # 配置加载
│       ├── modbus/             # Modbus 服务
│       ├── observability/      # 日志与链路跟踪
│       └── web/                # HTTP API 与 Scalar 文档
├── scripts/
│   └── install-modbus-manager.sh
├── dist-workspace.toml
└── README.md
```

## 许可证

MIT
