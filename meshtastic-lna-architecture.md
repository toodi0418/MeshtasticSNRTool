# Meshtastic LNA 測試系統架構說明書

> 平台：Tauri（Rust backend + Web frontend）  
> 介面：GUI（Tauri 視窗）＋ CLI（純終端）  
> 通訊：IP 或 Serial 115200  
> 測試對象：Meshtastic 網路中「頂樓節點 / 山上節點 / 車上或房間測試節點」

---

## 1. 整體架構總覽

### 1.1 高層結構

整套系統分為三層：

1. **Core / Engine（Rust Library）**
   - 負責所有「測試邏輯」與「通訊」：
     - 測試模式 / 拓樸（中繼 / 直連）狀態機
     - LNA ON/OFF 控制（頂樓 / 山上 / Local / Target）
     - traceroute 發送與結果解析
     - RSSI / SNR 樣本過濾與紀錄
     - 進度條與 ETA 計算
     - 資料輸出（CSV / JSON 等）

2. **桌面 GUI（Tauri App）**
   - 前端使用 TypeScript + Web 框架（例：React / Svelte）
   - 透過 Tauri command / event 呼叫 Rust Core：
     - 設定測試參數
     - 啟動 / 停止測試
     - 顯示即時進度、ETA、RSSI / SNR

3. **CLI 工具（Rust binary）**
   - 使用同一個 Rust Core Library
   - 從命令列參數讀取設定
   - 在終端印出文字版進度條 + ETA
   - 將測試結果寫入檔案

### 1.2 系統角色與節點

主要網路節點角色：

- **Local / 測試節點**
  - 實際執行 traceroute、作為「使用者端」
  - 連接方式：IP 或 Serial 115200
  - 可能位置：房間、車上（車頂吸盤天線）

- **Roof / 頂樓節點**
  - 中繼節點，位於頂樓
  - 在「中繼模式」中作為唯一中繼 hop
  - LNA 可遠端開關

- **Mountain / Target / 目標節點**
  - 通常是山上或某處固定節點
  - 在中繼模式為終點
  - 在直連模式為對端目標
  - LNA 可遠端開關（視你設定）

---

## 2. 測試模式與拓樸

### 2.1 拓樸模式

系統支援兩大拓樸：

1. **中繼模式（Relay Topology）**
   - 固定目標路徑：  
     `Local → Roof → Mountain`
   - 過濾條件：
     - hop 數必須為 1
     - 唯一中繼節點 ID = Roof 節點
   - 用途：
     - 測試頂樓 / 山上 LNA 對「中繼路徑」的影響

2. **直連模式（Direct Topology）**
   - 固定目標路徑：  
     `Local → Target`
   - 過濾條件：
     - hop 數必須為 0（完全直連）
   - 用途：
     - 車上 / 房間測試節點，直接打目標節點
     - 測試 local / target LNA 對直連 link 的影響
     - 或單純做 coverage 掃描（純量測 RSSI / SNR）

### 2.2 測試輪與情境

#### 中繼模式（Relay）

- **第 1 輪：Roof LNA 測試**
  - Mountain LNA 固定
  - Roof LNA 在 ON / OFF 間交替
  - 比較：
    - Mountain → Roof（山上 → 頂樓）SNR / RSSI
    - Roof → Mountain（頂樓 → 山上）SNR / RSSI

- **第 2 輪：Mountain LNA 測試**
  - Roof LNA 固定（例如 ON）
  - Mountain LNA 在 ON / OFF 間交替
  - 比較：
    - Roof → Mountain（頂樓 → 山上）SNR / RSSI
    - Mountain → Roof 作為參考

- **模式選擇：**
  - 只測第 1 輪（Roof）
  - 只測第 2 輪（Mountain）
  - 兩輪都測（先 Roof 後 Mountain）

#### 直連模式（Direct）

- **Local LNA 測試**
  - Target LNA 固定
  - Local LNA ON / OFF 交替
  - 比較：Local ↔ Target 雙向 RSSI / SNR

- **Target LNA 測試**
  - Local LNA 固定
  - Target LNA ON / OFF 交替

- **純掃描模式**
  - 不動任何 LNA
  - 固定 interval 做 traceroute
  - 將 Local ↔ Target 直連的 RSSI / SNR 持續紀錄（適合車上跑路測）

- **模式選擇：**
  - 只測 Local LNA
  - 只測 Target LNA
  - 兩邊都測
  - 純直連掃描（不動 LNA）

---

## 3. Rust Core / Engine 設計

### 3.1 主要元件

1. **Config / 設定物件**
   - 連線方式：
     - `transport_mode`: `Ip` / `Serial`
     - `ip`, `port`
     - `serial_port`（從清單選）
   - 拓樸：`topology`: `Relay` / `Direct`
   - 測試模式：`test_mode`：
     - relay: `RoofOnly`, `MountainOnly`, `Both`
     - direct: `LocalLna`, `TargetLna`, `Both`, `ScanOnly`
   - 測試參數：
     - `interval_ms`：traceroute 間隔
     - `phase_duration_ms`：單一 Phase 時長
     - `cycles`：OFF/ON 交替輪數
     - 直連純掃描時可用 `scan_duration_ms`
   - 節點資訊：
     - `local_node_id`
     - `roof_node_id`
     - `mountain_node_id`
     - `target_node_id`（直連用）
     - `lna_control_target`：Relay 模式可指定 `Disabled` / `Roof` / `Mountain` 來決定要不要遠端切換哪顆 LNA
   - 輸出設定：
     - `output_path`
     - `output_format`（csv / json 等）

2. **Transport 抽象層**
   - 介面概念：
     - 連線 / 中斷
     - 遠端設定 LNA
     - 執行 traceroute 並回傳結果
   - 實作：
     - IP 連線
     - Serial 115200（由 GUI 提供的下拉選項中選擇）

3. **測試引擎 / State Machine**
   - 依 `test_mode` / `topology` 控制：
     - 要跑哪些輪次（Roof / Mountain / Direct-*）
     - 各輪中的 OFF / ON Phase 切換
   - 在每個 Phase 期間：
     - 依 `interval_ms` 定時觸發 traceroute
     - 收到結果後過濾路徑
     - 抽出 RSSI / SNR
     - 寫入資料檔

4. **進度與狀態回報**
   - 計算：
     - `elapsed_ms` / `total_ms`
     - `progress_ratio`
     - `per_round_elapsed_ms` / `per_round_total_ms`
     - `round_progress_ratio`
     - `eta_timestamp`
   - 週期性將 `ProgressState` 回傳給：
     - Tauri backend（再送往前端）
     - CLI（終端進度列描畫）

5. **資料紀錄**
   - 每筆樣本建議欄位：
     - `timestamp`
     - `topology`
     - `test_round`
     - `direction`
     - `rssi`
     - `snr`
     - `roof_lna` / `mountain_lna`
     - `local_lna` / `target_lna`
   - 格式可自選 CSV / JSON Line。

### 3.2 路徑過濾邏輯

- Relay 模式：
  - 僅接受 `Local → Roof → Mountain`，hop = 1
- Direct 模式：
  - 僅接受 `Local → Target`，hop = 0
- 其他路徑一律標為無效樣本，不寫入正式結果。

---

## 4. GUI（Tauri Frontend）設計

### 4.1 設定畫面

1. 連線方式
   - IP 模式：輸入 IP / Port
   - Serial 模式：
     - 下拉選單列出可用 Serial 裝置
     - 提供「重新掃描」按鈕
     - 不允許手動輸入 Port 名稱

2. 拓樸模式
   - 中繼（Local → Roof → Mountain）
   - 直連（Local → Target）

3. 測試模式
   - 中繼：Roof / Mountain / Both
   - 直連：LocalLNA / TargetLNA / Both / ScanOnly

4. 時間參數
   - interval、phase duration、cycles、scan duration（視模式）

5. 節點設定
   - Local / Roof / Mountain / Target Node ID

6. 輸出設定
   - 檔案路徑、格式

7. 控制按鈕
   - 測試連線 / 開始 / 停止

### 4.2 進度與 ETA 顯示

- 總進度條（依 `progress_ratio`）
- 當前輪次進度條（依 `round_progress_ratio`）
- ETA / 剩餘時間
- 目前輪次 / Phase 狀態文本
- 最新 RSSI / SNR 樣本列表
- 錯誤與系統訊息區

---

## 5. CLI 設計重點

- 支援：
  - IP / Serial 模式
  - Relay / Direct 拓樸
  - 多種測試模式（Roof / Mountain / Both / Direct-*）
- 提供 `--list-serial` 列出當前可用 Serial 裝置
- 印出文字進度條與 ETA，例如：

  `[######--------------] 32% | 輪次 1/2：Roof LNA 測試 (Phase 2/4 OFF) | ETA 14:35 | 剩餘 08:23`

- 測試完成後顯示摘要與輸出檔案位置。

---

## 6. 錯誤處理與保護機制摘要

- 測試中禁止修改關鍵設定（連線模式 / 拓樸 / 測試模式）
- 連線失敗 / 中途斷線：立即停止測試並回報
- LNA 遠端設定多次失敗 → 中止該輪測試
- 路徑不符合條件只當無效樣本處理

---

## 7. 實作優先順序建議

1. 最小可用版本：
   - Relay + Roof 測試 + IP 連線 + CSV
   - 具有基本進度條與 ETA

2. 擴充：
   - 加入 Mountain 測試
   - 加入 Serial 支援與 Serial 下拉選單
   - 加入 Direct / 直連模式
   - 加入 CLI 介面

3. 進階：
   - 加入圖表 / DB / GPS 整合等。

---

此檔可直接作為專案的 `ARCHITECTURE.md`，後續若有規格變更再往下加註即可。
