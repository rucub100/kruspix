# Kruspix OS - Gemini AI Instructions

Welcome to the Kruspix project! To ensure you provide the best possible assistance, you must read and adhere to the context and rules defined in the following files before proceeding with any tasks.

## 🧭 Core Directives & Architecture
You MUST prioritize and strictly follow the architectural rules, conventions, and patterns defined in the main agents documentation:
*   **`AGENTS.md`**: (Located in the root directory). This is the primary source of truth for the kernel's architecture, `#![no_std]` constraints, hardware interactions, and critical design patterns (like `with_addr_lock` and driver dependency retries).

## 🗂️ Additional Instructions & Skills
Depending on the task, you should also look for specific instructions and specialized skills in these locations:
*   **`.agents/`**: Check this directory for subsystem-specific agent instructions.
*   **`.agents/skills/`**: Look here for custom skills and workflows tailored to this project.
*   **`.github/instructions/`**: Review this folder for repository-level guidelines.
*   **`.github/copilot-instructions.md`**: Review this file for rules that are also shared with GitHub Copilot.

## 🗺️ Project Context
To understand the overall project goals, current state, and future plans, refer to:
*   **`README.md`**: (Located in the root directory). For the general overview and setup instructions.
*   **`ROADMAP.md`**: (Located in the root directory). To understand planned features, upcoming milestones, and where current tasks fit into the bigger picture.