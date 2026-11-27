# 仕様書：Pure Rust 2段推論（PLaMo 3 Translate × Qwen3）＋ 外部JSON方言プラグイン アーキテクチャ

本書は、`candle-spike` で検証・実証した **100% Pure Rust** によるローカル LLM 2段推論パイプライン（**日→英翻訳 → 英語思考・内省コーチング → 英→日・方言翻訳**）を、Bullet プロジェクトへ再利用・移植するための技術仕様書です。

---

## 1. 背景とアーキテクチャ方針

### 1.1 解決する課題
1B〜2B クラスの軽量ローカル LLM（`Qwen3-1.7B` 等）に日本語のまま長文の内省コーチングや方言・キャラクター口調を同時に要求すると、以下の問題が発生しやすい。
- **コンテキスト・表現力の圧迫**: 方言の語彙や話し方の維持にモデルの注意（Attention）が奪われ、「共感＋本質的な問いを1つ立てる」というコーチングの論理構造が崩れやすい。
- **長文入力での破綻**: 日本語の長文・口語表現をそのまま推論させると、トークン消費が増え、問いを正しく立てられなくなるケースがある。

### 1.2 2段推論（責務分離）アプローチ
思考と翻訳・口調制御をモデルレベルで完全に分離し、以下の3ステップで処理する。

```text
[ユーザー入力 (日本語・長文/口語)]
        │
        ▼  Step 1: JA → EN (PLaMo 3 Translate 0.8B / PFML3)
[英語テキスト]
        │
        ▼  Step 2: 論理思考・コーチング (Qwen3-1.7B / 英語System Prompt)
[英語コーチング回答 (1文の共感 + 1つの具体的な質問)]
        │
        ▼  Step 3: EN → JA + 口調/方言転写 (PLaMo 3 Translate 0.8B / DialectConfig JSON)
[最終回答 (標準語・北海道道南弁・関西弁など任意のスタイル)]
```

### 1.3 技術スタックと省メモリ設計
- **100% Pure Rust**: C/C++ バインディング（`llama.cpp` 等）を一切使用せず、`candle-core` / `candle-nn` / `candle-transformers` と `tokenizers`（`onig` feature 有効）のみで構成する。
- **逐次ロード & 明示的 `drop` 設計**: モバイルや省メモリ環境でのピーク RAM を抑えるため、各ステップの推論完了ごとにモデル構造体を `drop(...)` してメモリを解放する（デスクトップ等でメモリに余裕がある場合は、常駐させて読込時間をゼロにする選択も可能）。

---

## 2. 依存クレートとビルド要件

### 2.1 `Cargo.toml` 必須依存関係
```toml
[dependencies]
candle-core = "0.11.1"
candle-nn = "0.11.1"
candle-transformers = "0.11.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokenizers = { version = "0.21", default-features = false, features = ["onig"] }
tracing = "0.1"
```
> **注意**: `tokenizers` の `onig` feature は、PLaMo 3 トークナイザーの Pre-tokenizer で後方参照を含む正規表現 `(.)\1{3,}` を評価するために必須。

### 2.2 ビルドプロファイルに関する注意点
`rustc 1.99.1`（LLVM 23.1）環境で `lto = true`（Fat LTO）や `opt-level = 3` を組み合わせると、リンク時または最適化時に `rustc` / `lld` が SIGSEGV する現象が確認されている。Bullet へ組み込む際、同様の環境で発生する場はバイナリクレートの LTO を無効化（`-C lto=off -C opt-level=2` または `lto = false`）してビルドすること。

---

## 3. PLaMo 3 (`plamo3`) GGUF ローダー仕様

`candle-transformers v0.11.1` には `plamo3` アーキテクチャが未実装のため、独自の GGUF ローダー（参考実装: `src/quantized_plamo3.rs`）を用いる。

### 3.1 対象 GGUF メタデータ仕様（`plamo-3-translate-0.8b-Q4_K_M.gguf` 検証済み値）
| メタデータキー | 型 | 実測値 (0.8B) | 用途 |
| :--- | :--- | :--- | :--- |
| `general.architecture` | String | `"plamo3"` | アーキテクチャ識別 |
| `plamo3.block_count` | U32 | `16` | Transformer レイヤー数 |
| `plamo3.embedding_length` | U32 | `1536` | `hidden_size` |
| `plamo3.feed_forward_length` | U32 | `6144` | `intermediate_size` |
| `plamo3.attention.head_count` | U32 | `16` | Query ヘッド数 (`num_heads`) |
| `plamo3.attention.head_count_kv` | U32 | `1` | KV ヘッド数 (`num_kv_heads`, MQA/GQA) |
| `plamo3.attention.key_length` | U32 | `128` | `head_dim`（`q_proj_dim = 2048 != hidden_size` に注意） |
| `plamo3.attention.layer_norm_rms_epsilon` | F32 | `1e-6` | RMSNorm の ε |
| `plamo3.attention.sliding_window` | U32 | `2048` | SWA ウィンドウサイズ |
| `plamo3.attention.sliding_window_pattern` | U32 | `8` | Full Attention 間隔 (`(i + 1) % pattern == 0`) |
| `plamo3.rope.freq_base` | F32 | `1000000.0` | Full Attention 層の RoPE 基底周波数 |
| `plamo3.rope.freq_base_swa` | F32 | `10000.0` | SWA 層の RoPE 基底周波数 |
| `plamo3.rope.scaling.type` | String | `"yarn"` | Full Attention 層の RoPE スケーリング |
| `plamo3.rope.scaling.factor` | F32 | `256.0` | YaRN スケーリング係数 |
| `plamo3.rope.scaling.original_context_length` | U32 | `4096` | YaRN 元のコンテキスト長 |
| `plamo3.rope.scaling.yarn_beta_fast` / `slow` | F32 | `32.0` / `1.0` | YaRN 周波数ランプパラメータ |

### 3.2 テンソル名と Forward パス固有の注意点
1. **結合 QKV 投影 (`blk.{i}.attn_qkv.weight`, shape `[2304, 1536]`)**:
   - 出力次元 `2304` を先頭から `Q (2048 = 16*128)`、`K (128 = 1*128)`、`V (128 = 1*128)` に `narrow` で分割する。
   - `q_proj_dim` (`2048`) は `hidden_size` (`1536`) と一致しないため、Attention 出力後の reshape と `attn_output.weight` (`[1536, 2048]`) は `q_proj_dim` 基準で扱う。
2. **Per-head QK-Norm**:
   - RoPE 適用前に `blk.{i}.attn_q_norm.weight` (`[128]`) と `blk.{i}.attn_k_norm.weight` (`[128]`) をヘッド単位で適用する。
3. **層ごとの RoPE 切り替え**:
   - `(layer_idx + 1) % sliding_window_pattern == 0`（0.8B では `layer_idx = 7, 15`）は **YaRN RoPE**（`freq_base` + concentration 係数 `0.1 * ln(factor) + 1.0`）、それ以外の層は **標準 NeoX-style RoPE**（`freq_base_swa`）を使用する。
4. **結合 SwiGLU MLP (`blk.{i}.ffn_up.weight`, shape `[12288, 1536]`)**:
   - `gate_proj` と `up_proj` が結合されているため、前半 `6144` 次元に SiLU を掛けて後半 `6144` 次元と要素積を取り、`blk.{i}.ffn_down.weight` (`[1536, 6144]`) に通す。
5. **Post-Norm と `.weight` サフィックスなしテンソル**:
   - `blk.{i}.post_attention_norm` と `blk.{i}.post_ffw_norm` は GGUF 上で `.weight` が付かない。
   - 残差接続は `x = residual + post_norm(sublayer(pre_norm(residual)))` の順で適用する。
6. **RMSNorm オフセットの事前加算（Fold）**:
   - `llama.cpp` 変換スクリプト（`conversion/plamo.py`）により、GGUF 内の RMSNorm 重みには既にオフセット（Pre-norm `+1.0`、`post_attention_norm` `+1/5`、`post_ffw_norm` `+1/5^1.5`）が加算済み。そのため、推論時は標準の `candle_transformers::quantized_nn::RmsNorm` でそのまま計算できる。

---

## 4. PLaMo 3 トークナイザー仕様

`pfnet/plamo-3-translate-0.8b` は `tokenizer.json` を持たず独自の Python トークナイザーを使用しているため、GGUF 埋め込みメタデータから Pure Rust で直接構築する（参考実装: `src/plamo3_tokenizer.rs`）。

1. **語彙構築**:
   - `tokenizer.ggml.tokens`、`tokenizer.ggml.scores`、`tokenizer.ggml.token_type` を読み込み、`tokenizers::models::unigram::Unigram` を生成する。
   - バイトトークン（`token_type == 6` かつ `score <= -1e6`）は Unigram の探索で破綻しないようスコアを `-100.0` 程度に正規化する。
2. **Pre-tokenizer 境界ルール**:
   - 2個以上の連続スペース: ` {2,}` を `SplitDelimiterBehavior::Isolated` で分割。
   - 4回以上の連続同一文字: `(.)\1{3,}` を `SplitDelimiterBehavior::Isolated` で分割（`onig` 必須）。
3. **特殊トークンとストップ判定**:
   - `<|plamo:*|>` 形式を `AddedToken` として登録（`bos_token_id=1`, `eos_token_id=1`, `eom_token_id=2`, `eot_token_id=16`）。
   - エンコード時は先頭に `bos_token_id` (`1`) を付与する。
   - 翻訳生成時のサンプリングは **`Sampling::ArgMax` (`temperature = 0.0`)** 固定。
   - 生成ループは `eos_token_id` / `eot_token_id` / `eom_token_id` への到達、または生成文字列に `"<|plamo:tag|>"` / `"<|plamo:bos|>"` が出現した時点で即座に停止し、特殊トークン以降をトリミングする。
4. **デコード（Byte Fallback）**:
   - `<0xXX>` 形式のトークンは生の `u8` バイト列へ戻し、UTF-8 として結合・復元する。

---

## 5. PFML3 プロンプトと外部 JSON 方言プラグイン仕様

### 5.1 基本プロンプト（Step 1: JA → EN）
```text
<|plamo:tag|>dataset<|plamo:msg|>translation<|plamo:tag|>input<|plamo:key|>lang<|plamo:val|>ja<|plamo:msg|>{日本語テキスト}<|plamo:tag|>output<|plamo:key|>lang<|plamo:val|>en<|plamo:msg|>
```

### 5.2 拡張プロンプト構造（Step 3: EN → JA + 方言・スタイル制御）
PLaMo 3 公式 Jinja テンプレートの PFML3 タグ構造に準拠し、以下の順序で連結する。

```text
<|plamo:tag|>dataset<|plamo:msg|>translation
[<|plamo:tag|>context<|plamo:msg|>{context}]
[<|plamo:tag|>input<|plamo:key|>lang<|plamo:val|>en<|plamo:msg|>{few_shot.en}<|plamo:tag|>output<|plamo:key|>lang<|plamo:val|>ja<|plamo:msg|>{few_shot.ja}] ...
[<|plamo:tag|>glossary<|plamo:index|>{entry.source}<|plamo:item|>{entry.target} ...]
<|plamo:tag|>input<|plamo:key|>lang<|plamo:val|>en<|plamo:msg|>{en_text}
<|plamo:tag|>output<|plamo:key|>lang<|plamo:val|>ja[<|plamo:key|>form<|plamo:val|>{form}]<|plamo:msg|>
```

### 5.3 外部 JSON スキーマ（`DialectConfig`）
Bullet プロジェクトでは、方言・キャラクター口調ごとに以下の JSON ファイルを配置して動的に読み込む。

```json
{
  "name": "モード表示名（例: 北海道・道南弁（やるべしたらモード））",
  "form": null,
  "context": "方言や話し方のトーン指示（任意）",
  "glossary": [
    { "source": "Sakutto", "target": "Sakutto" },
    { "source": "Question:", "target": "ちょっと聞くけどさ：" },
    { "source": "How many", "target": "いくつくらい" }
  ],
  "few_shots": [
    {
      "en": "英語のお手本入力",
      "ja": "対応する方言のお手本出力"
    }
  ]
}
```

#### スキーマ設計・運用のベストプラクティス
- **`form` フィールド**: 標準語・敬体の場合は `"polite"`、常体の場合は `"plain"`、方言やキャラ口調を使う場合は **`null`** にする（`"polite"` を指定すると方言の語尾と競合して敬体へ引き戻されるため）。
- **`glossary` による固有名詞保護**: アプリ名やプロジェクト名（例: `"Sakutto"`, `"Bullet"`）をそのまま残したい場合は `{ "source": "Bullet", "target": "Bullet" }` と登録する。
- **`glossary` による問いの構造維持**: `"Question:"` や `"How many"` のようなコーチングの核となる表現を方言訳語へアンカーしておくことで、意味の欠落を防ぎつつ自然な方言へ変換できる。
- **`few_shots`（1〜2件）**: モデルの文体・語尾・温かみを安定させるため、共感1文＋質問1文の対訳例を 1〜2 件入れるのが最も効果的。

---

## 6. Bullet プロジェクトへの移植・モジュール設計ガイド

Bullet 側で再利用しやすいよう、以下のように責務を分割して組み込むことを推奨する。

1. **`plamo3` クレート / モジュール（翻訳エンジン層）**
   - `Plamo3ModelWeights`（GGUF ロード・推論・`clear_kv_cache`）
   - `Plamo3Tokenizer`（GGUF メタデータからの Unigram + Onig 分割構築）
   - `pfml3::DialectConfig`（JSON ローダーと PFML3 ビルダー）
2. **`coach` モジュール（思考エンジン層）**
   - `Qwen3`（`candle_transformers::models::quantized_qwen3::ModelWeights`）を用いた英語プロンプト推論。
   - システムプロンプトと 1-shot 例示は英語で記述し、出力フォーマット（1文の共感＋1つの具体的な質問）を固定する。
3. **`pipeline` オーケストレーター層**
   - ユーザー設定から選択された `DialectConfig`（JSON パス）を受け取り、`JA -> EN -> Coaching(EN) -> JA(Dialect)` を実行する。
   - メモリ制約の厳しいターゲット環境では、本スパイク同様に各ステップ間でモデルを `drop` してピークメモリを抑制し、メモリに余裕のあるデスクトップ環境では両モデルをキャッシュしてレイテンシを削減する。

---

## 7. 検証済みベンチマーク参考値（CPU 実行時）

`candle-spike`（CPU 実行、`plamo-3-translate-0.8b-Q4_K_M.gguf` + `Qwen3-1.7B` GGUF）での実測値：

| ステップ | モデル | 読込時間 | 生成速度 | 備考 |
| :--- | :--- | :--- | :--- | :--- |
| Step 1: JA → EN (長文4行) | PLaMo 3 Translate 0.8B | ~1.4s | ~25.6 – 26.6 tok/s | 88 → 85 tokens |
| Step 2: EN Coaching | Qwen3-1.7B | ~2.8 – 6.3s | ~9.7 – 10.6 tok/s | 247 → 30 tokens |
| Step 3: EN → JA (標準語) | PLaMo 3 Translate 0.8B | ~1.2s | ~25.4 – 26.2 tok/s | `form="polite"` |
| Step 3: EN → JA (方言JSON) | PLaMo 3 Translate 0.8B | ~1.2s | ~23.7 – 24.8 tok/s | `glossary` 7件 + `few_shots` 2件 |

---

## 8. 参照ソースファイル（`candle-spike` 内）
- `src/quantized_plamo3.rs` — PLaMo 3 GGUF ローダー・Forward パス実装
- `src/plamo3_tokenizer.rs` — GGUF 埋め込み語彙トークナイザー・PFML3 / `DialectConfig`
- `src/main.rs` — 2段推論パイプライン・CLI 引数（`--dialect`）・計測ロジック
- `src/bin/inspect_gguf.rs` — GGUF メタデータ・テンソル検査ツール
- `dialects/*.json` — 標準語・北海道道南弁・関西弁の外部設定サンプル
