# Enhanced Downloader Architecture

This document outlines the architecture for the enhanced downloader, focusing on Static Analysis (SWC-style) and comprehensive bypass systems.

## 1. Static Analysis Engine (SWC-style)

The goal is to analyze HTML and JavaScript content without a full browser or Node.js environment.

### Components:
- **HTML Parser**: A fast, low-memory HTML parser (using `lol-html` or `html5ever`).
- **JS Expression Evaluator**: A lightweight JavaScript expression evaluator for handling obfuscated links or dynamic URL generation (using `boa_engine` or a custom regex-based evaluator for simple cases).
- **Rule Engine**: A flexible system to define extraction patterns using CSS selectors and regex.

## 2. Comprehensive Bypass System

This system aims to protect the downloader from tracking, fingerprinting, and blocking.

### Components:
- **Network Layer**:
    - **DNS-over-HTTPS (DoH)**: Bypass ISP DNS filtering and prevent DNS leaks.
    - **Proxy Support**: Support for SOCKS5 and HTTP proxies.
    - **Custom TLS Stack**: Use `rustls` or `native-tls` with custom ALPN and cipher suites to mimic real browsers.
- **Fingerprinting Protection**:
    - **User-Agent Rotation**: Use a pool of realistic User-Agents.
    - **Header Randomization**: Randomize header order and values (e.g., `Accept-Language`, `Sec-CH-UA`).
    - **Canvas/WebRTC/Audio Fingerprint Spoofing**: Inject scripts or modify responses to provide fake fingerprinting data.
- **Privacy & Anonymity**:
    - **Cookie Management**: Isolated cookie jars per request/session.
    - **Referer Spoofing**: Set realistic referers based on the target platform.
    - **Timezone/Language Spoofing**: Match headers with the expected location.

## 3. Integration & Optimization

- **Low RAM Usage**: Use streaming parsers and avoid loading large files into memory.
- **Fast Performance**: Leverage Rust's concurrency model and asynchronous I/O.
- **Low Maintenance**: Use a declarative rule system for easy updates.

## 4. Implementation Plan

1.  **Phase 3**: Implement the Static Analysis engine.
2.  **Phase 4**: Implement the bypass systems.
3.  **Phase 5**: Integrate all components into the `leech-core` engine.
4.  **Phase 6**: Comprehensive testing.
