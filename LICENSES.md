# Licence files — which one is normative

The repository contains four licence files, inherited from the original project plus one note. This file exists
because their relationship was never stated (audit defect D7), and a distributor should not have to guess.

| File | What it is | Normative? |
| --- | --- | --- |
| [LICENSE](LICENSE) | The full text of the **GNU General Public License, version 2** | **Yes** — this is the licence Morid is distributed under |
| [LICENSE.txt](LICENSE.txt) | The original author's copyright notice and grant: Copyright (c) 2021–2026, Francis Banyikwa, GPL-2.0-or-later | **Yes** — the copyright notice is not removable and must accompany any distribution |
| [GPLv2](GPLv2) | A second copy of the GPLv2 text, as shipped by the original project | No — duplicate of `LICENSE`; kept because the original shipped it |
| [GPLv3](GPLv3) | The text of the GNU General Public License, version 3 | No — included because the licence is "version 2 **or later**" and a distributor choosing the "later" option needs the text |

## What this means in one paragraph

Morid is free software under **GPL-2.0-or-later**: you may redistribute it and/or modify it under the terms of
the GNU General Public License as published by the Free Software Foundation, either version 2 of the licence, or
(at your option) any later version. It comes with no warranty. If you distribute a modified version, the GPL
applies to your version too, and the copyright notices of both Francis Banyikwa (the original program) and the
Morid contributors must be retained.

## Third-party components

Components that are **linked** into the binaries, and tools that are **run** as separate processes, are listed
with their licences in [THIRD-PARTY.md](THIRD-PARTY.md). Two points from there are worth repeating here:

- **Slint** (the UI toolkit) is used under its royalty-free licence, which requires attribution — kept in the
  About window and in `THIRD-PARTY.md`. This does not change Morid's own licence.
- **wget is GPL-3.0** and **ffmpeg builds with GPL components are GPL-2.0-or-later**. Both are separate
  programs invoked over a command line, not linked libraries. Do not link them into the Morid binaries: that
  would change the licence of the combined work.

## Changing the licence

Not a pull request. It requires an analysis of code provenance — the project is a derivative work of a
GPL-2.0-or-later program — and an owner decision recorded in [docs/decisions.md](docs/decisions.md). See
[CONTRIBUTING.md](CONTRIBUTING.md) §8.
