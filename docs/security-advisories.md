# Security Advisories and CVEs

This policy covers private security reports, CVE handling, GitHub Security
Advisories, embargoes, coordinated disclosure, and release publication
requirements.

## Severity Triage

Initial severity is assigned during private triage and may change as
reproduction evidence improves.

| Severity | Criteria |
| --- | --- |
| Critical | Reliable allocator memory corruption, arbitrary code execution, or cross-thread metadata corruption affecting a supported release line with ordinary safe Rust or documented C ABI use. |
| High | Use-after-free, double-free, invalid free, metadata corruption, predictable hardening secrets, or panic/unwind across FFI affecting a supported release line and reachable through plausible application inputs. |
| Medium | Denial of service, leak, hardening bypass, unsupported-target issue with a likely maintained-target impact, or vulnerability requiring unusual but documented unsafe/raw usage. |
| Low | Security-relevant correctness issue with narrow impact, unsupported-target-only issue, documentation error, or issue requiring invalid caller behavior outside the documented safety contract. |

Triage must record:

- affected versions, release lines, target platforms, artifact modes, and
  features,
- whether the issue reproduces on Linux x86_64 GNU,
- whether C mimalloc 3.3.2 reproduces the same behavior,
- whether the issue is a supported API/ABI bug, raw-caller contract violation,
  unsupported-target portability issue, or hardening gap,
- severity, affected surfaces, mitigations, fixed versions, and whether a CVE
  is requested.

## CVE Handling

Request or reserve a CVE when a vulnerability affects a published release line
or a downstream-distributed release artifact. A master-only pre-release issue
normally does not need a CVE unless maintainers know it was distributed by a
downstream or the reporter, maintainer, or GitHub Security Advisory process
requires one.

The advisory must record the CVE ID when one is assigned. If no CVE is assigned,
the advisory should still publish the GitHub Security Advisory identifier when
the issue affects a released version.

## Embargo Handling

Reports stay private until a fix, release, advisory text, and disclosure plan
are ready. The default embargo target is up to 90 calendar days after enough
information is available to reproduce and assess severity. Shorter disclosure is
preferred when a fix is ready, the issue is already public, or the reporter
requests it.

Embargo extensions require reporter agreement and a written reason in the
private advisory. Public commits, CI logs, issue titles, changelog drafts, and
release branches must not disclose vulnerability details before coordinated
publication.

## Coordinated Disclosure

For confirmed vulnerabilities:

1. Keep discussion in the GitHub Security Advisory or another private channel
   agreed with the reporter.
2. Assign a maintainer owner for severity, fix scope, release scope, and
   disclosure timing.
3. Identify affected versions, supported release lines, artifact modes,
   features, target platforms, workarounds, and whether C mimalloc 3.3.2 is
   affected.
4. Prepare the fix in a private security branch or private advisory fork when
   public work would reveal exploit details.
5. Run the release validation required by `docs/release.md` for every fixed
   supported line.
6. Prepare release notes, changelog entry, advisory text, credits, and CVE/GHSA
   metadata before publication.
7. Publish fixed releases, tags, checksums, release notes, changelog, and the
   advisory in the same disclosure window.

If the issue is determined not to be a vulnerability, close the private report
with the rationale and move any non-sensitive follow-up to public issues when
appropriate.

## Changelog and Advisory Publication

Every published security fix must update `CHANGELOG.md`, release notes, and the
GitHub Security Advisory.

The changelog `Security` entry must include:

- CVE ID or GitHub Security Advisory ID,
- severity,
- affected versions and first fixed version,
- affected API, ABI, artifact mode, feature, or target surface,
- user-visible impact,
- mitigation or upgrade guidance,
- reporter credit when the reporter consents.

The GitHub Security Advisory must include:

- summary and severity,
- affected package and version ranges,
- patched versions,
- workaround or mitigation status,
- CVE/GHSA identifiers,
- enough technical detail after embargo lift for users to assess exposure,
- links to release notes, changelog, signed checksums, and fixed commit or tag.
