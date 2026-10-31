# OpenCode Migrator (agc)

跨平台（Windows 與 Linux）的 **OpenCode** 工作階段遷移工具，基於 [Ratatui](https://ratatui.rs/) 提供現代化終端互動介面（TUI），亦具備強大的命令列模式（CLI）。

能夠將 OpenCode 的對話紀錄一鍵遷移至 **Freebuff** 或 **AGY (Antigravity)**，或同時遷移至兩者。

---

## TUI 互動介面操作

直接執行 `agc`

```bash
agc
```

### 鍵盤快捷鍵

| 按鍵                   | 功能說明                                                                                |
| :--------------------- | :-------------------------------------------------------------------------------------- |
| `↑` / `↓` 或 `k` / `j` | 上下移動選取工作階段                                                                    |
| `Space`（空白鍵）      | 勾選 / 取消勾選工作階段（多選模式）                                                     |
| `p` / `P` 或 `v`       | 💬 **對話預覽（Preview）**：彈窗查看完整聊天紀錄、思考過程與時間戳（支援上下/翻頁滾動） |
| `a`                    | 全選 / 取消全選所有工作階段                                                             |
| `Tab` 或 `←` / `→`     | 切換遷移目標平台：`[1] Freebuff` → `[2] AGY` → `[3] Both`                               |
| `1` / `2` / `3`        | 直接指定遷移目標：`1` 為 Freebuff、`2` 為 AGY、`3` 為兩者皆要                           |
| `d`                    | 切換模式：`[COPY]` 安全複製 或 `[MOVE]` 遷移後刪除來源                                  |
| `Enter` 或 `m`         | 開始執行遷移（針對已選取項目，未選取則遷移當前遊標項目）                                |
| `r`                    | 重新讀取 OpenCode 資料庫與工作階段清單                                                  |
| `q` 或 `Esc`           | 退出程式                                                                                |

---

## CLI 命令列模式

如果不進入 TUI，也可以透過命令列直接批次執行：

### 1. 列出所有工作階段

```bash
agc list
```

### 2. 遷移特定工作階段

```bash
# 遷移至 Freebuff
agc move --target freebuff --session-id ses_f2734f87affeV7xhJ2jHZazxQ4

# 遷移至 AGY
agc move --target agy --session-id ses_f2734f87affeV7xhJ2jHZazxQ4

# 同時遷移至 Freebuff 與 AGY
agc move --target both --session-id ses_f2734f87affeV7xhJ2jHZazxQ4
```

### 3. 一鍵遷移所有工作階段

```bash
agc move --target agy --all
```

### 4. 移動模式（遷移後刪除 OpenCode 原始紀錄）

```bash
agc move --target freebuff --session-id <SESSION_ID> --delete-source
```

### 5. 模擬執行（Dry-run）

```bash
agc move --target both --all --dry-run
```

### 6. 自訂資料路徑

```bash
agc --opencode-db /path/to/opencode.db --freebuff-dir /path/to/freebuff --agy-dir /path/to/antigravity-cli
```

---

## 編譯與安裝

### 需求條件

- [Rust 1.75+](https://rustup.rs/)

### 從原始碼安裝

```bash
# 複製專案庫
git clone https://github.com/ChiuHuang/opencode-migrator.git
cd opencode-migrator

# 編譯並安裝至 ~/.cargo/bin
cargo install --path .
```

安裝完成後，確認 `~/.cargo/bin` 位於系統的 `PATH` 中，即可在任何路徑直接使用 `agc` 指令！

---

## 授權條款

本專案採用 [MIT License](LICENSE) 授權。
