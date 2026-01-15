# Changelog

All notable changes to OxideC will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0-alpha] - 2026-01-16

### Added
- Categories: Dynamic method addition to existing classes
- Protocols: Protocol definition with inheritance and hybrid validation
- Protocol conformance: Declarative (default) and optional runtime validation
- Protocol inheritance: Protocols can extend other protocols
- Transitive protocol conformance through class inheritance
- Message Forwarding: Per-object, per-class, and global forwarding hooks
- Forwarding loop detection to prevent infinite forwarding chains
- Method Swizzling: Runtime method replacement with atomic operations
- Cache invalidation on method swizzle
- Integration tests: 16 new tests for forwarding and swizzling

### Changed
- Updated all documentation to reference Cargo.toml as version source
- Restructured documentation to separate concerns (README, RFC, ARCHITECTURE, SAFETY)
- Centralized roadmap tracking in RFC.md
- Added comprehensive Phase Status summary to RFC.md
- Updated ARCHITECTURE.md to reference RFC.md for status and test counts
- Updated SAFETY.md to reference RFC.md for MIRI validation status
- Updated CLAUDE.md to reference Cargo.toml and RFC.md

### Testing
- 148 unit tests (all passing)
- 16 integration tests (7 forwarding + 9 swizzling, all passing)
- 74 doctests (68 passing, 6 ignored)
- Total: 238 tests
- MIRI validated with `-Zmiri-strict-provenance -Zmiri-ignore-leaks`

### Dependencies
- No new dependencies (continues to have no external dependencies)

### Documentation
- Created CHANGELOG.md for release history tracking
- Updated all documentation files to use single source of truth architecture
- Cargo.toml is now the single source of version information
- RFC.md is now the single source for roadmap, test counts, and phase status

## [0.2.0] - 2026-01-15

### Added
- Message dispatch with method caching
- Type encoding system for Objective-C compatibility
- Complete object and class lifecycle management
- Selector interning and O(1) lookup caching
- Thread-safe reference counting with atomic operations
- Argument marshalling with type encoding
- Return value handling with unaligned access support
- Method resolution order: local -> categories -> superclass
- Method cache invalidation on dynamic updates
- Thread-safe class/selector/protocol creation
- Concurrent method registration with RwLock protection

### Testing
- 61 unit tests (all passing)
- MIRI validated with strict provenance

## [0.1.0] - 2026-01-15

### Added
- Initial release with foundation features
- Arena allocator for metadata
- Runtime strings with Small String Optimization (SSO)
- Basic object and class system
- Core runtime infrastructure
- Selector interning system
- Method registry implementation
- Class creation and registration
- Object allocation and deallocation
- Reference counting with atomic operations

### Testing
- 42 unit tests (all passing)
- MIRI validated with strict provenance

---

## Links
- [Cargo.toml](Cargo.toml) - Current version
- [RFC.md](RFC.md) - Roadmap and development status
- [ARCHITECTURE.md](ARCHITECTURE.md) - Design and architecture
- [SAFETY.md](SAFETY.md) - Safety guidelines
