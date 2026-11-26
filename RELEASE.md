# Release process

**No release exists.** Nothing has been published, signed or uploaded.

Planned process (to be implemented in Phase 6–7, each step requiring owner
approval where marked):

1. Tag a version on `main` after STATUS.md shows the release's features as
   real-platform tested.
2. CI builds installers on Windows and macOS runners from the tag.
3. Code signing — **requires owner approval and certificates**:
   * Windows: Authenticode certificate (paid) or Azure Trusted Signing (paid).
   * macOS: Apple Developer Program membership (paid) for Developer ID signing
     and notarisation.
   Unsigned builds will be labelled as such; customers will not be told to
   disable security protections.
4. Generate SHA-256 checksums and a machine-readable release manifest
   (artifact name, version, architecture, size, URL, checksum).
5. Install, launch, scan, cleanup (disposable files) and uninstall on real
   machines for every advertised platform; record evidence in STATUS.md.
6. Website download buttons are generated from the manifest only.
7. Publish — **requires owner approval**.
