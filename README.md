# Meshtastic LNA Test Tool (MSNRTool)

這是一個用於測試 Meshtastic 節點 LNA (Low Noise Amplifier) 效能的自動化工具。
支援透過 IP 或 Serial 連線控制節點，並執行自動化的 Traceroute 測試以收集 RSSI 與 SNR 數據。

## 功能特點

- **多種拓樸支援**：
    - **Relay 模式**：測試頂樓/山上中繼節點 (Local -> Roof -> Mountain)。
    - **Direct 模式**：測試直連節點 (Local -> Target) 或進行覆蓋範圍掃描。
- **自動化測試流程**：
    - 自動切換遠端節點的 LNA (ON/OFF)。
    - 定時發送 Traceroute 並記錄路徑品質。
    - 支援多輪測試與循環。
- **雙介面**：
    - **CLI**：輕量級終端工具，適合腳本化與無頭運作。
    - **GUI** (開發中)：Tauri 應用程式，提供視覺化設定與圖表。

## 系統架構

- **Core (`msnr-core`)**：Rust 核心函式庫，包含狀態機、設定定義與 Meshtastic 協定實作。
- **CLI (`msnr-cli`)**：命令列介面。
- **App (`msnr-app`)**：Tauri + React 前端介面。

## 安裝與執行

### 前置需求
- Rust (最新 stable)
- Node.js (用於 GUI 建置)

### CLI 使用方式

1. **編譯**：
   ```bash
   cargo build --release
   ```

2. **執行測試**：
   ```bash
   # 透過 IP 連線
   ./target/release/msnr-cli run --transport ip --ip 192.168.1.100

   # 透過 Serial 連線
   ./target/release/msnr-cli run --transport serial --serial /dev/ttyUSB0
   ```

3. **查看說明**：
   ```bash
   ./target/release/msnr-cli --help
   ```

## 開發狀態

- [x] 專案結構初始化
- [x] 核心邏輯 (Config, Engine)
- [x] Meshtastic 協定整合 (IP/Serial)
- [x] CLI 基礎功能
- [ ] GUI 介面實作 (Tauri)
- [ ] 真實硬體測試驗證

## 授權

MIT
