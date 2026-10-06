# ClipOn Security Policy & Credential Handling

## 1. Security Overview

ClipOn is built with a defense-in-depth security model to ensure user data, media assets, and sensitive cloud API credentials remain confidential and protected.

---

## 2. Credential Management & OS Keyring

All sensitive credentials are stored securely via operating system credential stores:

- **macOS**: Apple Keychain Services via `keyring` crate.
- **Windows**: Windows Credential Manager.
- **Linux**: Secret Service API.

### IPC Protection

- API keys are **never** passed from the frontend to the backend as arguments in long-running job operations.
- The backend resolves keys directly from the OS Keyring when executing cloud transcription or LLM calls.
- Keys are sanitized and redacted from all debug and error logging outputs.

---

## 3. Safe Process & Command Execution

ClipOn strictly forbids executing commands through shell interpreters (`sh -c` or `bash -c`).

- All external tool invocations (`ffmpeg`, `ffprobe`, `yt-dlp`, `clipon-face-tracker`) use direct `std::process::Command::new(binary).args(&[...])`.
- Arguments are passed as individual array elements, completely eliminating shell argument injection and command injection vulnerabilities.

---

## 4. Path Traversal & Filesystem Safety

- All filesystem operations validate input paths against directory bounds.
- Media file extensions are strictly checked (`validate_media_extension`).
- Temporary rendering artifacts are written to isolated `TempDirGuard` directories within system temp storage and automatically deleted upon drop or app startup.

---

## 5. YouTube & Copyright Policy

- Before initiating media downloads via `yt-dlp`, ClipOn checks the video license metadata for Creative Commons or reuse permissions.
- Non-Creative Commons videos display a prominent Copyright Advisory banner.
- Users must explicitly confirm acknowledgment of YouTube's Terms of Service and their legal right to process the content.

---

## 6. Secrets & Git Repository Hygiene

The following items are strictly excluded from version control:

- `.env` files and environment dumps.
- Application databases (`clipon.db`, `autoshorts.db`).
- Rendered video exports and downloaded source files.
- Personal filesystem paths and machine usernames.

---

## 7. Reporting Security Vulnerabilities

If you discover a security issue or vulnerability in ClipOn, please report it directly via GitHub Private Vulnerability Reporting or contact the maintainer at [https://github.com/subha-3128/ClipOn/security](https://github.com/subha-3128/ClipOn/security).
