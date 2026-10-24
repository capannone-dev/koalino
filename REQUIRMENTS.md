# REQUIREMENTS.md - Project Specifications & Constraints

## 1. Project Overview
This project is a decentralized, offline-first, AI-powered meeting note-taking application. It operates over a peer-to-peer (P2P) network, dynamically routes AI workloads based on device capabilities, and provides seamless cross-device synchronization with end-to-end encryption (E2EE).

## 2. Platform & UI/UX Requirements
The application MUST be deployed across the following platforms, adhering to their specific native design guidelines:
*   **Android:** Mobile-optimized, Material Design 3.
*   **Linux Desktop:** Native look and feel for GNOME (Debian/Arch based).
*   **Windows Desktop:** Native Windows UI guidelines.
*   **Linux Server:** Headless daemon with a modern, responsive Web UI.
*   **Linux CLI:** Full-featured command-line interface for headless/server environments.

### 2.1 Background & Lock Screen Operations
*   The app MUST continue recording audio in the background.
*   On mobile devices, the app MUST provide Start, Pause, and Stop recording controls directly from the lock screen or notification shade without requiring device unlock.

## 3. Core Functional Requirements

### 3.1 Audio Recording & Media Handling
*   **Capture:** MUST capture high-quality audio. MUST detect screen sharing (e.g., via MS Teams) and automatically capture screenshots of slides/informative content.
*   **Storage Policy:** Once a meeting is fully transcribed and processed, the raw audio recording MUST be automatically deleted to save space and protect privacy. Screenshots and text notes MUST be retained.
*   **Attachments:** Users MUST be able to add attachments (PDFs, photos) to a meeting after the fact. The system MUST analyze file metadata (timestamps, location) to automatically suggest which meeting the attachment belongs to.

### 3.2 AI Voice Diarization & Identity Management
*   **Diarization:** MUST distinguish between different speakers.
*   **Voice Signature Library:** MUST maintain a library of voice embeddings/signatures linked to user identities.
*   **Handling Audio Variations:** Because mic/speaker setups alter voice profiles, the system MUST support multiple voice signatures per identity.
*   **Manual Intervention & Learning:**
    *   If a voice is unmatched, the user MUST be able to manually assign it to a new or existing identity.
    *   If a match has low confidence, the system MUST prompt the user to confirm or correct it.
    *   If confirmed, the system MUST generate and store a *new* signature variant for that identity to improve future matching in similar audio environments.
    *   Unassigned voice clips MUST be temporarily preserved so the user can listen to them and identify the speaker.
*   **Management UI:** MUST include a dedicated "Voice Gallery" page to manage identities and voice signatures.

### 3.3 Meeting Notes & Synchronization
*   **Metadata:** All notes MUST be automatically tagged with Date, Time, and Acquisition Device ID.
*   **Offline-First Sync:** The system MUST function 100% offline. When a node reconnects, it MUST seamlessly synchronize its repository with all other peers without data loss or complex manual merge conflicts.
*   **Shared State:** Voice signature libraries and meeting repositories MUST be shared across the P2P network.

### 3.4 Distributed AI Compute & Routing
*   **Device Manifests:** Every node MUST broadcast a manifest detailing its hardware capabilities (e.g., GPU VRAM, CPU cores, available models).
*   **Workload Routing:** Users MUST be able to select where AI workloads run: `Local Device`, `Specific Remote Device`, or `Auto` (system selects the best fit based on manifests).
*   **Extensibility:** The architecture MUST allow swapping local AI models for 3rd-party API models (e.g., OpenAI, Anthropic) without changing core business logic.
*   **"Dumb" Nodes:** The architecture MUST accommodate future "dumb" nodes that only capture and stream raw audio to the P2P network without running local AI.

### 3.5 Telemetry & Statistics
*   The system MUST collect and group performance statistics per device and per AI task.
*   Metrics MUST include: Transcription processing time, estimated power consumption (crucial for mobile), network latency, and compute load.

## 4. Non-Functional Requirements

### 4.1 Networking & Security
*   **P2P Topology:** The network MUST be strictly Peer-to-Peer. A node going offline MUST NOT block or crash other nodes.
*   **Connectivity:** MUST work over local LAN (mDNS) and over the internet (NAT traversal/hole punching).
*   **Encryption:** ALL communication between devices MUST be End-to-End Encrypted (E2EE). Voice signatures (biometric data) MUST be encrypted at rest.

### 4.2 Language Support
*   The AI transcription and NLP models MUST be optimized for **Italian** and **English**.

## 5. Code Quality, Documentation & Testing Standards
*These rules are strictly enforced. Code that violates these rules will be rejected.*

### 5.1 Documentation Hierarchy
*   **Root:** A main `README.md` providing a project overview, installation instructions, and links to sub-docs.
*   **Directories:** Every folder MUST contain a `README.md` explaining its specific role in the architecture.
*   **Files:** Every source file MUST start with a high-level summary comment block.
*   **Functions/Methods:** MUST include docstrings detailing: Inputs, Outputs, Intended Use, Implementation details, and Side Effects.
*   **Variables:** MUST use highly descriptive, significant names. Complex variables require inline comments.
*   **Context Files:** `ARCHITECTURE.md`, `API_SPECS.md`, and `CONTEXT.md` MUST be maintained and kept up to date.

### 5.2 Testing & TDD (Test-Driven Development)
*   **TDD Mandate:** Tests MUST be written *before* or *alongside* the implementation. 
*   **Coverage:** Tests MUST cover general use cases, edge cases, and misuse/failure cases.
*   **API/Protocol Testing:** Any defined protocol or API MUST have specific tests on both the client and server sides. A third party MUST be able to integrate using only the API documentation and tests, without seeing the internal code.
*   **Pre-Commit Gate:** ALL tests MUST pass before a commit is finalized.

### 5.3 Git & Workflow Rules
*   **Repository:** All code MUST be committed to a private GitHub repository.
*   **Micro-Commits:** Commits MUST be small, atomic, and incremental. (e.g., Do not commit "Add P2P". Commit "Add mDNS local peer discovery", then "Add NAT traversal").
*   **Commit Messages:** MUST clearly document the changes between commits.

## 6. Definition of Done (DoD)
A feature is only considered "Done" when:
1. The code is implemented and passes all linting/formatting checks.
2. Unit and Integration tests are written and passing.
3. All required documentation (File headers, function docstrings, folder READMEs) is updated.
4. The `CONTEXT.md` or `TASK_BOARD.md` is updated to reflect the new state.
5. The changes are committed following the micro-commit strategy.