# Octo-Avenger (Screenly Player System)

## Overview

This is the complete Screenly player system consisting of multiple components including the viewer, client software, device management services, and deployment infrastructure.

## Project Structure

### Core Components

- **viewer/**: Qt/QML viewer application for media playback
- **client/**: Python client services and device management
- **rust-services/**: High-performance Rust services for device management, networking, and playlist handling
- **disk-image-generator/**: Tools for creating Screenly OS images
- **snaps/**: Snap packaging configurations for different architectures

### Key Services

- **Device Manager**: Rust service for device configuration and D-Bus communication
- **Network Watchdog**: Network connectivity monitoring
- **Playlist Service**: Content downloading and management
- **Log Collector**: System log aggregation and reporting

## Viewer Integration

### Intro Asset Support

The viewer has comprehensive intro asset support for device onboarding:

- **Documentation**: See `viewer/CLAUDE.md` for detailed viewer architecture
- **Design Reference**: [Device Setup Figma Design](https://www.figma.com/design/o9iRZLIJCGmYwUjlpkzJe3/Device-Setup---Sergey?node-id=0-1&p=f&t=ZlbGtwfMyL0ldSHB-0)
- **Implementation**: Complete intro screen with loading states, device pairing, and setup wizard
- **Location**: `viewer/screenly-client/qml/intro/LoadScreenUnified.qml`

### Recent Updates

✅ **Intro Page Integration** (2025-07-30)

- Ported comprehensive intro design from separate `/intro` directory to main viewer
- Updated `IntroPage.qml` to use new `LoadScreenUnified` component
- Created dedicated intro asset structure in `screenly-client/qml/intro/`
- Added QML resource definitions for intro assets and fonts
- Fixed asset paths to use Qt resource system (`qrc:/intro/...`)

## Architecture Notes

- Multi-language system: Qt/QML (viewer), Python (client), Rust (services)
- Snap-based deployment for cross-platform compatibility
- D-Bus communication between services
- Asset-based content system with specialized surface types

## Coding Standards

### General Principles

**⚠️ CRITICAL: Early Returns and Minimal Indentation**

This is a **mandatory** coding standard that applies to **ALL languages** in the codebase:

- **C++** - destructors, methods, functions
- **Rust** - functions, async functions, error handling
- **Python** - all functions and methods
- **QML/JavaScript** - all component logic and functions
- **CMake** - control flow in build scripts
- **Any other language** used in this project

**Rules:**

- **ALWAYS use early returns** to reduce nesting
- **Maximum indentation: 2-3 levels** - deeper nesting indicates refactoring needed
- **Guard clauses first** - validate and return/exit early
- **Invert conditions** - check for error/empty/null cases first, then return early
- **Avoid nested conditionals** - flatten logic whenever possible

**Examples:**

```cpp
// ✅ Good - C++ destructor with early return
ProbeCliProcess::~ProbeCliProcess() {
    if (!process_) {
        return;
    }

    if (process_->state() != QProcess::NotRunning) {
        process_->kill();
        process_->waitForFinished(1000);
    }
    process_->deleteLater();
}

// ❌ Avoid - Nested if statements
ProbeCliProcess::~ProbeCliProcess() {
    if (process_) {
        if (process_->state() != QProcess::NotRunning) {
            process_->kill();
            process_->waitForFinished(1000);
        }
        process_->deleteLater();
    }
}

// ✅ Good - Check for error cases first
if (jsonData.isEmpty()) {
    emit error("No output from probe-cli");
    return;
}

QJsonDocument doc = QJsonDocument::fromJson(jsonData);
// ... continue processing

// ❌ Avoid - Nesting the success case
if (!jsonData.isEmpty()) {
    QJsonDocument doc = QJsonDocument::fromJson(jsonData);
    // ... lots of nested logic here
} else {
    emit error("No output from probe-cli");
}
```

### Rust Code Style

- **Early Returns**: ALWAYS use early returns to reduce nesting
- **Guard Clauses**: Prefer guard clauses over nested if statements
- **Error Handling**: Use `?` operator and early returns for error conditions
- **Readability**: Minimize indentation levels - maximum 2-3 levels preferred
- **Formatting**: Always run `cargo fmt` before committing

Example:

```rust
// ✅ Good - Early returns
async fn process_data(&self) -> Result<String, Error> {
    let connection = Connection::system().await?;

    let proxy = match self.create_proxy(&connection).await {
        Ok(p) => p,
        Err(e) => return Err(ProbeError::NetworkInterface(format!("Proxy failed: {}", e))),
    };

    let data = proxy.get_data().await?;
    if data.is_empty() {
        return Ok("No data".to_string());
    }

    Ok(data)
}

// ❌ Avoid - Nested conditions
async fn process_data(&self) -> Result<String, Error> {
    if let Ok(connection) = Connection::system().await {
        if let Ok(proxy) = self.create_proxy(&connection).await {
            if let Ok(data) = proxy.get_data().await {
                if !data.is_empty() {
                    return Ok(data);
                }
            }
        }
    }
    Err(Error::Failed)
}
```

### Python Code Style

- **Early Returns**: ALWAYS use early returns to reduce nesting
- **Type Annotations**: REQUIRED for all new code
- **Modern Type Syntax**: Use `int | None` instead of `Optional[int]`
- **Type Checking**: Run `pyright` on modified files
- **Formatting**: Use `ruff format` before committing

Example:

```python
# ✅ Good - Modern type annotations with early returns
def process_data(value: str | None) -> dict[str, Any]:
    if value is None:
        return {}

    data = parse_value(value)
    if not data:
        return {}

    return {"result": data}

# ❌ Avoid - Old-style Optional and nested conditions
from typing import Optional, Dict, Any

def process_data(value: Optional[str]) -> Dict[str, Any]:
    if value is not None:
        data = parse_value(value)
        if data:
            return {"result": data}
    return {}
```

## Development Workflow

### After Making Changes

After implementing changes, follow this workflow (unless changes are UI-only):

#### 1. Add Tests (When Possible)

- **Rust**: Add unit tests and integration tests
- **C++**: Add tests using the `test_client` binary
- Tests should cover new functionality and edge cases

#### 2. Format Code

Always format code before committing:

**Rust:**

```bash
cargo fmt
```

**QML:**

```bash
qmlformat <file.qml>
```

**C++:**

```bash
clang-format -i <file.cpp>
```

**CMake:**

```bash
cmake-format -i <CMakeLists.txt>
```

**Python:**

```bash
ruff format <file.py>
```

**Markdown:**

```bash
dprint fmt <file.md>
```

#### 3. Run Tests and Type Checks

**Rust:**

```bash
cargo nextest run
```

**C++:**

```bash
# Use test_client binary to run specific tests
./test_client [test-name]
```

**Python:**

```bash
# Type check modified files
pyright <file.py>
```

### Language-Specific Tools

| Language | Formatter      | Test Runner         | Type Checker | Notes                                 |
| -------- | -------------- | ------------------- | ------------ | ------------------------------------- |
| Rust     | `cargo fmt`    | `cargo nextest run` | Built-in     | Always format before commit           |
| QML      | `qmlformat`    | N/A                 | N/A          | UI code formatting                    |
| C++      | `clang-format` | `test_client`       | N/A          | Binary for running specific tests     |
| CMake    | `cmake-format` | N/A                 | N/A          | Build system formatting               |
| Python   | `ruff format`  | TBD                 | `pyright`    | Use modern type syntax: `int \| None` |
| Markdown | `dprint fmt`   | N/A                 | N/A          | Run automatically on all .md changes  |

## Getting Started

1. See `kb/development_setup.md` for development environment setup
2. See `viewer/CLAUDE.md` for viewer-specific documentation
3. See `kb/building_screenly_client.md` for build instructions
