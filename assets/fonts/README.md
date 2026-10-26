# Fonts

Inter and JetBrains Mono are embedded in `disphas-report` so that certificates render offline and identically
everywhere. The application embeds Atkinson Hyperlegible Next (interface), JetBrains Mono (technical data) and
Klaudia Bold (the word « Disphas » only, `klaudia/`): it never loads a font from the network.

| File | Family | Subset | Source |
|---|---|---|---|
| `Inter-Regular-latin.ttf` | Inter 400 | latin | `cdn.jsdelivr.net/fontsource/fonts/inter@latest/latin-400-normal.ttf` |
| `Inter-Regular-latin-ext.ttf` | Inter 400 | latin-ext | `…/inter@latest/latin-ext-400-normal.ttf` |
| `Inter-Bold-latin.ttf` | Inter 700 | latin | `…/inter@latest/latin-700-normal.ttf` |
| `Inter-Bold-latin-ext.ttf` | Inter 700 | latin-ext | `…/inter@latest/latin-ext-700-normal.ttf` |
| `JetBrainsMono-Regular-latin.ttf` | JetBrains Mono 400 | latin | `…/jetbrains-mono@latest/latin-400-normal.ttf` |
| `JetBrainsMono-Regular-latin-ext.ttf` | JetBrains Mono 400 | latin-ext | `…/jetbrains-mono@latest/latin-ext-400-normal.ttf` |
| `AtkinsonHyperlegibleNext-{Regular,SemiBold,Bold}-latin.ttf` | Atkinson Hyperlegible Next 400, 600, 700 | latin | `…/atkinson-hyperlegible-next@latest/latin-{400,600,700}-normal.ttf` |
| `AtkinsonHyperlegibleNext-{Regular,SemiBold,Bold}-latin-ext.ttf` | idem | latin-ext | `…/atkinson-hyperlegible-next@latest/latin-ext-{400,600,700}-normal.ttf` |

Inter and JetBrains Mono downloaded on 2026-10-04, Atkinson Hyperlegible Next on 2026-10-26 (licence from
`github.com/google/fonts`, `ofl/atkinsonhyperlegiblenext/OFL.txt`) from the Fontsource packages. Licensed under the SIL Open Font
License 1.1: see `LICENSE-Inter.txt`, `LICENSE-JetBrainsMono.txt` and `LICENSE-AtkinsonHyperlegibleNext.txt`.

Coverage: Latin and Latin Extended (French, German, most European names). Other scripts
(Cyrillic, Greek, CJK…) typed in free-text fields are not covered yet.
