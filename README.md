# Yarubeshitara! (`yarube`) — Ultra-Lightweight CLI Daily Todo & Reflection

**English** | [日本語 (Japanese)](README.ja.md)

**Yarubeshitara! (やるべしたら！)** is an ultra-fast, zero-dependency CLI Todo & Reflection notebook written in Rust.
Capture your daily **"△ Todo List"** and **"△ Reflection"** from the terminal in under 0.1 seconds with the effortless feel of a pocket paper notepad.

---

## Why I Built This (and What "Yarubeshitara!" Means)

> I wanted the effortless feel of the paper notepad I use every single day, combined with the convenience of the terminal—so I built this completely for myself! It's a lightweight Todo & reflection app laser-focused on speed and daily usability.

**Why the name `Yarubeshitara!` (`yarube`)?**
*"Yarubeshitara!" (やるべしたら！)* is a warm, energetic phrase from the **Hokkaido dialect** of northern Japan meaning **"Well then, let's do this!"** or **"Let's get to it!"**. Typing `yarube` in the terminal is a small daily nudge to turn thoughts into action.

---

## Key Features

- **Zero-Friction Capture (< 0.1s)**
  - Run `yarube Prepare slides for tomorrow` (no subcommand or quotes needed) to immediately append `• Prepare slides for tomorrow` to today's Todo List and redraw the page with a `[NEW]` highlight badge.
  - Attach indented context/notes to the preceding task with `yarube sub "Compare Lubuntu -> Xubuntu"`.
- **1-Key End-of-Day Review & Automatic Migration (`yarube review`)**
  - Walk through pending tasks (`•`) one by one with a single keystroke: `o` (Done `O`), `m` or `>` (Migrate to tomorrow `>`), `c` (Cancel `~`), or `s` (Skip).
  - Tasks marked as Migrated (`>`) are automatically carried over—along with all of their indented subtasks (`  - ...`)—to tomorrow's journal as pending tasks (`•`).
  - Progress is saved atomically after every single item, so pressing `Ctrl+C` mid-review never loses confirmed updates.
- **Free-Form Daily Reflection (`yarube reflect`)**
  - Configure your favorite GUI or terminal editor (`code --wait`, `nvim`, `vim`, `nano`, etc.) via `yarube config editor` and dump your thoughts at the end of the day, or append a quick one-liner directly from the CLI.
- **Page-Turning Navigation & Full-Text Search**
  - Flip back to your previous recorded day (automatically skipping unrecorded weekends/holidays) with `yarube prev` (`yarube -1`, `yarube -2`), move forward with `yarube next` (`yarube +1`), or flip pages interactively with `yarube browse`.
  - Search across all past daily logs with `yarube search <keyword>` to extract matching task blocks and reflection lines with keyword highlighting.
- **4 True Color Themes & Safe Unicode Symbols**
  - Built-in 24-bit True Color themes: **`tokyo-night`** (default), **`monokai`**, **`nord`**, and **`sepia`**. Switch and preview instantaneously with `yarube theme <name>`.
  - Uses only universally supported standard Unicode symbols (`•`, `>`, `O`, `~`, `-`, `─`, `△`)—zero Nerd Fonts or emoji dependencies, guaranteeing zero garbled characters (`mojibake`) across any terminal or OS.
- **Bilingual (English / Japanese) & Zero External Crates**
  - Automatically adapts UI messages and headers to your `LANG` locale (or explicit `yarube config lang en|ja` setting). The Markdown parser seamlessly reads both English (`#### △ Todo List` / `#### △ Reflection`) and Japanese (`#### △ やることリスト` / `#### △ ふりかえり`) headings.
  - Built using **100% Rust standard library (`std`)** with zero external crate dependencies.

---

## Journal Format (Plain Markdown)

Each day is stored as a plain Markdown file at `~/.yarube/YYYY-MM-DD.md` (customizable via `$YARUBE_DIR`):

```markdown
### 10/17(Tue)

#### △ Todo List
O Write tests for SIT-1-1-1-001
> Spin up GitBucket on Podman
  - Compare Lubuntu -> Xubuntu
• Build conversion tool for MCP
  - Convert PDF -> Markdown

#### △ Reflection
Switched to EndeavourOS!
UX matters. Everything runs super smoothly and lighter than Win11.
Books are much easier to read now—time to study hard.
```

### Status Symbols

| Symbol | Status | Description |
| :---: | :--- | :--- |
| `•` | **Todo** | Newly added task, or task migrated from the previous day |
| `O` | **Done** | Completed task (rendered with Dim + Strikethrough styling) |
| `>` | **Migrated** | Task migrated to the next day's Todo List |
| `~` | **Cancelled** | Dropped / cancelled task (rendered with Dim + Strikethrough styling) |
| `  - ` | **Subtask** | Indented note under a parent task (migrates together with its parent) |

---

## Installation

Because `yarube` has zero third-party crate dependencies, it compiles offline in ~1 second with standard `cargo`:

```bash
git clone <repository-url>
cd Bullet
cargo build --release

# Copy the binary to a directory in your PATH (e.g. ~/.local/bin)
cp target/release/yarube ~/.local/bin/yarube
```

---

## Usage

### 1. Daytime Rapid Logging

```bash
# View today's journal (auto-generates template on first run of the day)
yarube

# Add a task (with or without 't', unquoted multiple words are joined automatically)
yarube t Write tests for SIT-1-1-1-001
yarube Prepare slides for tomorrow

# Add an indented subtask / note under the most recent task
yarube sub "Convert PDF -> Markdown"
```

### 2. Evening Review & Reflection

```bash
# Interactive 1-key review of pending tasks (•)
# Keys: [o] Done(O)  [m/>] Migrate(>)  [c/~] Cancel(~)  [s/Enter] Skip(•)  [q/Ctrl+C] Quit
yarube review

# Open your configured editor to write in '△ Reflection'
yarube reflect

# Or append a reflection line directly from the CLI
yarube reflect Great focus session today!
```

### 3. Page Navigation & Search

```bash
# Flip to previous/next recorded days (skips unrecorded days/holidays)
yarube prev     # or: yarube -1
yarube -2       # 2 recorded days back
yarube next     # or: yarube +1

# Interactive page-turning mode ([p/h/←] Prev  [n/l/→] Next  [t] Latest  [q] Quit)
yarube browse

# Full-text search across all daily logs with keyword highlighting
yarube search EndeavourOS
```

### 4. Themes, Editor & Language Configuration

```bash
# Switch color theme and immediately preview today's journal
yarube theme tokyo-night
yarube theme monokai
yarube theme nord
yarube theme sepia
yarube theme              # Interactive menu

# Configure reflection editor (Priority: config file > $VISUAL > $EDITOR > nano/notepad)
yarube config editor
yarube config editor "code --wait"

# Switch display language (English / Japanese)
yarube config lang en
yarube config lang ja
```

---

## Color Themes Palette (24-bit True Color)

| UI Element | Style | Tokyo Night (`tokyo-night`) | Monokai (`monokai`) | Nord (`nord`) | Sepia (`sepia`) |
| :--- | :--- | :---: | :---: | :---: | :---: |
| **Date Header** (`### MM/DD(Day)`) | Bold | `#7aa2f7` | `#66d9ef` | `#88c0d0` | `#d4a373` |
| **Section Heading** (`#### △ ...`) | Bold | `#bb9af7` | `#fd971f` | `#81a1c1` | `#bc6c25` |
| **Pending Task** (`•`) | Normal | `#c0caf5` | `#f8f8f2` | `#eceff4` | `#faedcd` |
| **Done / Cancelled** (`O` / `~`) | Dim + Strikethrough | `#565f89` | `#75715e` | `#4c566a` | `#8c8275` |
| **Migrated Task** (`>`) | Normal | `#e0af68` | `#e6db74` | `#ebcb8b` | `#b08968` |
| **Subtask / Note** (`  - `) | Normal | `#9aa5ce` | `#a6e22e` | `#d8dee9` | `#ddb892` |
| **Reflection Body** | Normal | `#c0caf5` | `#f8f8f2` | `#eceff4` | `#faedcd` |
| **`[NEW]` Badge** | Bold | `#9ece6a` | `#a6e22e` | `#a3be8c` | `#6b705c` |
| **Divider Line** (`─`) | Dim | `#3b4261` | `#49483e` | `#3b4252` | `#d5bdaf` |

---

## Architecture & Testing

`yarube` is structured using a clean **Ports & Adapters (Layered) Architecture** for high cohesion, loose coupling, and testability:

- `src/domain/`: Pure, I/O-free domain logic (`date`, `journal`, `config`, `navigation`, `search`, `i18n`, `theme`)
- `src/ports.rs`: Trait abstractions (`JournalRepository`, `ConfigRepository`, `EditorLauncher`)
- `src/infra/`: Atomic filesystem storage (`~/.yarube/YYYY-MM-DD.md`, `~/.yarube/config`) and OS editor process launcher
- `src/ui/` & `src/cli.rs`: Pure CLI argument parser, 24-bit True Color renderer, and interactive terminal controllers

```bash
# Run all 14 unit tests + 21 BDD integration tests (TP-01 through TP-21)
cargo test
```

---

## Column: How This Project Was Built (Antigravity × WDD)

This application was developed **without any custom agent harnesses, pre-configured rules, or custom skills**. Instead, it was built through conversational pair-programming with **Antigravity** driven by a single structured requirements file: [Requirements.md](Requirements.md).

We used **WDD (Why-Driven Development)** to structure requirements as a hierarchical Mermaid Mindmap that traces every feature from its core human motivation down to executable specifications:

1. **US (User Story — *Why*)**: Why the user wants the feature (e.g., *"to dump thoughts at the same low friction as a handwritten '△ Todo List'"*)
2. **AC (Acceptance Criteria)**: What conditions must be satisfied
3. **TP (Test Perspective)**: The specific quality and boundary perspectives to verify
4. **BDD (Behavior-Driven Development)**: Concrete `Given / When / Then` scenarios
5. **EARS (Easy Approach to Requirements Syntax)**: Unambiguous system behavior rules (Event-driven, State-driven, Ubiquitous, Unwanted behavior)

By evolving [Requirements.md](Requirements.md) step-by-step from `US-01` to `US-06` and aligning on the architecture before each phase, Antigravity translated all 21 Test Perspectives (`TP-01`–`TP-21`) directly into automated integration tests and a clean, zero-dependency Rust codebase.
