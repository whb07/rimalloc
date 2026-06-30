# Release Signing

Release signing uses SSH Ed25519 signatures for both source tags and staged
binary checksums.

## Mechanism

Source releases must use signed annotated Git tags. Configure Git for SSH
signing and create the tag with the release SSH signing key:

```sh
git config gpg.format ssh
git config user.signingkey /path/to/release-signing-key.pub
git config gpg.ssh.allowedSignersFile /path/to/release-allowed-signers
git tag -s vX.Y.Z -m "mechanical_mimalloc vX.Y.Z"
git tag -v vX.Y.Z
```

Binary release assets are authenticated by signing `SHA256SUMS`, not each file
individually. `scripts/build-release-artifacts.py` writes:

- `SHA256SUMS`
- `SHA256SUMS.sig`
- `integrity/signing/release-signing.pub`
- `integrity/signing/release-allowed-signers`

The signature namespace is `file`, the default signer identity is
`mechanical_mimalloc_release`, and the manifest mechanism string is
`ssh-ed25519-detached-sha256sums`.

## Release Key

The release owner controls the SSH Ed25519 release signing key. Keep the
private key outside the repository and inject it into release jobs through a
protected secret or release-only signing environment.

Set `MECHANICAL_MIMALLOC_RELEASE_SIGNING_KEY` to the private-key path before
building release artifacts:

```sh
MECHANICAL_MIMALLOC_RELEASE_SIGNING_KEY=/path/to/release-signing-key \
  scripts/check-release-integrity.py
```

`MECHANICAL_MIMALLOC_RELEASE_SIGNING_IDENTITY` may override the signer identity
when a future release line needs a distinct principal. The release notes must
publish the public key fingerprint shown in `release-artifacts.json`.

When no signing key is configured, the dry run generates an ephemeral test-only
Ed25519 key in a temporary directory outside the repository and deletes the
private key after signing. That key proves the signing and verification path
works, but it is not a release trust root and must not be used for published
assets.

## Verification

Verify the staged checksum signature with:

```sh
ssh-keygen -Y verify \
  -f target/release-artifacts/integrity/signing/release-allowed-signers \
  -I mechanical_mimalloc_release \
  -n file \
  -s target/release-artifacts/SHA256SUMS.sig \
  < target/release-artifacts/SHA256SUMS
```

Then verify the checksums from the release asset directory:

```sh
sha256sum -c SHA256SUMS
```

For downloaded release assets, use a trusted allowed-signers file or public key
fingerprint from the release notes. Do not treat an allowed-signers file from an
untrusted asset bundle as the trust root.

## Rotation

Key rotation is release-blocking until it is documented in the release notes.
For one transition release, publish both the old-key and new-key fingerprints,
sign `SHA256SUMS` with the new key, and include an additional old-key
attestation or tag signature when practical. Remove the old key from the trusted
allowed-signers set only after that transition release is published.
