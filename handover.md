# MSNR Tool - Project Handover

## 1. Project Overview
**MSNR Tool** (Meshtastic Signal-to-Noise Ratio Tool) is a specialized utility designed to test and optimize Meshtastic node placement. It automates the process of toggling LoRa settings (specifically the LNA / RX Boosted Gain) and measuring the resulting SNR values from a target node.

### Core Features
- **Automated Testing Engine**: Cycles through "LNA OFF" and "LNA ON" phases.
- **Remote Admin Control**: Toggles settings on remote nodes via the Mesh.
- **Traceroute Analysis**: Collects SNR and RSSI data hop-by-hop.
- **LNA 差異分析**: 針對 ON/OFF 兩個 phase 累積平均 SNR，並計算差值做為測試總結。
- **Cross-Platform**: Runs as a CLI tool or a GUI Desktop App (Windows/macOS/Linux).

---

## 2. System Architecture
The project is organized as a Rust Workspace with three main members:

### Directory Structure
```
MSNRTool/
├── core/           # [Library] The brain. Contains business logic and Meshtastic protocol handling.
├── cli/            # [Binary] Command-line interface for headless execution.
├── app/            # [Tauri] GUI application (React + TypeScript frontend).
├── target/         # Rust build artifacts.
└── output/         # Test results (CSV).
```

### Component Details

#### 2.1 Core Library (`msnr-core`)
- **`engine.rs`**: The state machine. Manages the test lifecycle (Start -> Phase 1 -> Phase 2 -> Finish) and data collection.
- **`transport/`**:
    - **`ip.rs`**: Handles TCP connection to Meshtastic devices (`172.16.x.x` or USB-over-TCP). Implements **PKI Encryption** for Admin commands.
    - **`serial.rs`**: (Stub/Partial) For direct USB serial connections.
- **`config.rs`**: Defines test parameters (Duration, Cycles, Target IDs).

#### 2.2 CLI (`msnr-cli`)
- A wrapper around `msnr-core`.
- Usage: `msnr-cli run --target !867263da --ip 172.16.8.92`
- Useful for scripting and headless servers (e.g., Raspberry Pi).

#### 2.3 GUI App (`msnr-app`)
- **Backend (Rust)**: `app/src-tauri`. Exposes `start_test`, `stop_test`, `get_status` commands to the frontend.
- **Frontend (TS/React)**: `app/src`.
    - **Framework**: Vite + React + TypeScript.
    - **Styling**: TailwindCSS / CSS Modules.
    - **Visualization**: Real-time progress bars and status logs.
    - **LNA Stats Card**: 顯示 OFF/ON 兩組平均 SNR、樣本數與差值（收到首筆有效 SNR 後即更新）。

---

## 3. Critical Technical Mechanisms

### 3.1 Admin Authorization (The "Identity Mismatch" Fix)
One of the hardest challenges was authorizing the tool to control a remote node (Roof Node) when connected via a local relay (Local Node).

- **Problem**: The Roof Node (`!867263da`) only authorized the User's Phone App ID (`!1493e609`). The Local Node had a different Hardware ID (`!a80dcc18`).
- **Discovery**: The Local Node **actually held the Private Key** corresponding to the App ID (`EP7u...`).
- **Solution**:
    We implemented `send_admin` in `ip.rs` with `pki_encrypted = true`.
    This forces the Firmware to sign the packet using its stored Private Key.
    The packet is sent *through* the Local Node, but *signed* as the Authorized App.
    
    ```rust
    // core/src/transport/ip.rs
    let mesh_packet = MeshPacket {
        to: dest, 
        // ...
        pki_encrypted: true, // <--- CRITICAL: Enables Firmware-side Signing
        payload_variant: Some(mesh_packet::PayloadVariant::Decoded(data)),
    };
    ```

### 3.2 Automated Test Logic
The `Engine` loop works as follows:
1. **Connect**: Establishes TCP connection to the Local Node.
2. **Phase 1 (LNA OFF)**:
    - Sends `AdminMessage` to set `sx126x_rx_boosted_gain = false`.
    - Verification: Reads back config to ensure it applied.
    - Loop: Sends Traceroute requests every N seconds.
    - Collects SNR data from responses.
3. **Phase 2 (LNA ON)**:
    - Sets `sx126x_rx_boosted_gain = true`.
    - Repeats data collection與平均統計。
4. **Report**: Saves all data to `results.csv`.
5. **Summary**: CLI 與 GUI 會顯示 OFF/ON 平均值與差值；若任一 LNA 設定/驗證階段超時則立即中止，確保測試結果可靠。

---

## 4. Setup & Build Instructions

### Prerequisites
- **Rust**: `rustup update`
- **Node.js**: `v18+`
- **Meshtastic Device**: Must be on same network (WiFi) or USB.

### Build Core & CLI
```bash
# In project root
cargo build --release --bin msnr-cli
```

### Build GUI App
```bash
cd app
npm install
npm run tauri build
```

### Development Mode (GUI)
```bash
cd app
npm run tauri dev
```

---

## 5. Status & Next Steps

### Completed
- [x] Core Engine Logic (Phases, Retry, Fail-safe).
- [x] TCP Transport with Protobufs.
- [x] Admin Authorization (PKI Encryption).
- [x] CLI Tool (Fully functional).
- [x] Basic GUI (Config Form + Progress).

### Pending / Future Work
- [ ] **Live Charts**: Visualize SNR over time in the GUI.
- [ ] **Serial Transport**: Finish implementing `send_admin` for USB Serial.
- [ ] **Map View**: Show traceroute hops on a map.
- [ ] 更進階的平均統計（例如 RSSI、標準差），以及在 GUI/CSV 中匯出。

## 6. Recent Updates
- 2025-12-11：GUI Dashboard 的「LNA 平均值比較」卡片僅在整個測試完全結束且 ON/OFF 都累積到樣本後才會顯示，避免測試進行中佔版面。
- 2025-12-11：移除 Engine 層級的 `Sending traceroute` log，僅保留 transport 端輸出，避免 CLI 出現兩行重複訊息。
- 2025-12-11：Relay 模式新增路徑驗證，僅接受 Local → Roof → Mountain 共三節點的樣本；路徑若多/少 hop 或未設定必填 Node ID 會直接丟棄資料，確保 SNR 平均值不被錯誤拓撲污染。
