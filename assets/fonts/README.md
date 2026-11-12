# Fonts

Klaudia (text), JetBrains Mono (data) and Inter (only for a character Klaudia lacks) are embedded in
`disphas-report` so that certificates render offline and identically everywhere. The application embeds Atkinson Hyperlegible Next (interface), JetBrains Mono (technical data) and
Klaudia Bold (the word « Disphas » only, `klaudia/`): it never loads a font from the network.

| File | Family | Subset | Source |
|---|---|---|---|
| `Inter-Regular-latin.ttf` | Inter 400 | latin | `cdn.jsdelivr.net/fontsource/fonts/inter@latest/latin-400-normal.ttf` |
| `Inter-Regular-latin-ext.ttf` | Inter 400 | latin-ext | `…/inter@latest/latin-ext-400-normal.ttf` |
| `Inter-Bold-latin.ttf` | Inter 700 | latin | `…/inter@latest/latin-700-normal.ttf` |
| `Inter-Bold-latin-ext.ttf` | Inter 700 | latin-ext | `…/inter@latest/latin-ext-700-normal.ttf` |
| `JetBrainsMono-Regular-latin.ttf` | JetBrains Mono 400 | latin | `…/jetbrains-mono@latest/latin-400-normal.ttf` |
| `JetBrainsMono-Regular-latin-ext.ttf` | JetBrains Mono 400 | latin-ext | `…/jetbrains-mono@latest/latin-ext-400-normal.ttf` |
| `AtkinsonHyperlegibleNext-{Regular,SemiBold,Bold}-latin.woff2` | Atkinson Hyperlegible Next 400, 600, 700 | latin | `…/atkinson-hyperlegible-next@latest/latin-{400,600,700}-normal.ttf` |
| `AtkinsonHyperlegibleNext-{Regular,SemiBold,Bold}-latin-ext.woff2` | idem | latin-ext | `…/atkinson-hyperlegible-next@latest/latin-ext-{400,600,700}-normal.ttf` |

Inter and JetBrains Mono downloaded on 2026-10-04, Atkinson Hyperlegible Next on 2026-10-26 (licence from
`github.com/google/fonts`, `ofl/atkinsonhyperlegiblenext/OFL.txt`) from the Fontsource packages. Licensed under the SIL Open Font
License 1.1: see `LICENSE-Inter.txt`, `LICENSE-JetBrainsMono.txt` and `LICENSE-AtkinsonHyperlegibleNext.txt`.

Coverage: Latin and Latin Extended (French, German, most European names). Other scripts
(Cyrillic, Greek, CJK…) typed in free-text fields are not covered yet.

The application loads WOFF2 files (2026-11-12, for the weight of the website's demo), converted from the TTF
files above with fontTools 4.68.0 (`fonttools ttLib --flavor woff2`), glyphs unchanged: Atkinson Hyperlegible
Next (the TTF files were removed) and `JetBrainsMono-Regular-latin*.woff2` (the TTF files stay for the
certificates). `klaudia/Klaudia-Bold-disphas.woff2` holds only the letters of « Disphas »
(`pyftsubset Klaudia-Bold.ttf --text=Disphas --layout-features='*' --flavor=woff2`): the application sets no
other word in Klaudia.
