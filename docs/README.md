# OxideUtils documentation

**Product:** OxideUtils  
**Vendor:** [Zainium Dynamics](https://zainiumdynamics.tech)  
**Contact:** alizain@zainiumdynamics.tech  
**Licence:** GPLv3 only  

Start here if you need more than the root [README.md](../README.md).

---

## Guides

| Doc | Audience | Content |
|-----|----------|---------|
| [architecture.md](./architecture.md) | Engineers | Crates, modules, data flow |
| [building.md](./building.md) | Everyone | `make`, cargo, CI, artifacts |
| [tools.md](./tools.md) | Users | Full CLI reference |
| [api-core.md](./api-core.md) | Kernel & lib users | `oxideutils-core` API |
| [std-no-std.md](./std-no-std.md) | Kernel + host | Dual-mode design |
| [kernel-integration.md](./kernel-integration.md) | Kernel team | Embed core in Zainium kernel |
| [gnu-compatibility.md](./gnu-compatibility.md) | Packagers / QA | Compatibility vs GNU **2.46.1** |
| [AUDIT-REPORT-BINUTILS-2.46.1.md](./AUDIT-REPORT-BINUTILS-2.46.1.md) | Architects | Full audit + risk + roadmap |
| [faq.md](./faq.md) | Everyone | Common questions |
| [configuration.md](./configuration.md) | Users / packagers | **TOML** config (`true`/`false`) |
| [release-process.md](./release-process.md) | Maintainers | Tag + GitHub release checklist |
| [man/](./man/) | Unix man pages | Source for `man oxide-*` |

## Project meta

| Doc | Content |
|-----|---------|
| [../ROADMAP.md](../ROADMAP.md) | 12-phase plan |
| [../CHANGELOG.md](../CHANGELOG.md) | Versions |
| [../CONTRIBUTING.md](../CONTRIBUTING.md) | Patches |
| [../SECURITY.md](../SECURITY.md) | Security policy |
| [../ARCHITECTURE.md](../ARCHITECTURE.md) | Architecture overview |

## Quick links

```bash
cd oxideutils
# edit oxideutils.toml, then:
cargo build --release
./target/release/oxide-objdump -v
```
