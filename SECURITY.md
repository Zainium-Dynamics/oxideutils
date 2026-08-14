# Security Policy — OxideUtils (Zainium Dynamics)

## Supported versions

| Version    | Supported          |
|------------|:------------------:|
| 0.1.x      | :white_check_mark: |
| < 0.1      | :x:                |

## Reporting a vulnerability

If you discover a security issue in OxideUtils, **please do not open a
public issue**. Instead, email us directly:

- **Email:** [alizain@zainiumdynamics.tech](mailto:alizain@zainiumdynamics.tech)
- **Subject line:** `[SECURITY] OxideUtils — <brief description>`

We will acknowledge your report within **48 hours** and aim to provide a
fix or mitigation within **7 business days** for confirmed issues.

## Scope

OxideUtils processes untrusted binary inputs (ELF, PE, Mach-O, Wasm,
archives). Security-relevant areas include:

- **Parser crashes / panics** on malformed input
- **Out-of-bounds reads** (even in safe Rust, logic bugs can cause
  incorrect slicing)
- **Denial of service** via resource exhaustion (memory, CPU) on crafted
  files
- **Incorrect mutation** (`strip`/`objcopy`) producing silently corrupt
  output

## What is NOT in scope

- GNU binutils bugs (we are not GNU — report those upstream)
- Feature requests or compatibility gaps
- Issues in third-party dependencies (report to the dependency authors,
  but let us know if it affects OxideUtils users)

## Disclosure

We follow coordinated disclosure. Once a fix is released, we will credit
the reporter (unless they prefer anonymity) in the GitHub release notes.

## Thank you

We appreciate researchers who help make OxideUtils safer. Zainium
Dynamics is committed to building memory-safe tooling for the ecosystem.
