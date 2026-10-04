// Oblivio batch certificate (docs/certificat.md §6.1).
//
// A summary of the drives erased in one batch. Each drive keeps its own
// signed attestation, attached to this PDF: the attestations are the proof,
// this document is their rendering. Layout only: every text arrives
// translated in `sys.inputs`, and nothing typed by an operator is evaluated.

#let d = sys.inputs

#let ink = rgb("#111827")
#let muted = rgb("#6B7280")
#let rule = rgb("#D1D5DB")
#let green = rgb("#10B981")
#let red = rgb("#EF4444")
#let indigo = rgb("#6366F1")

#set document(
  title: d.title + " — " + d.batch_id,
  date: datetime(
    year: d.year, month: d.month, day: d.day,
    hour: d.hour, minute: d.minute, second: d.second,
  ),
)
#set page(
  paper: "a4",
  margin: (x: 1.5cm, top: 1.6cm, bottom: 1.8cm),
  footer: context text(size: 7.5pt, fill: muted)[
    #d.title · #d.batch_id #h(1fr) #counter(page).display("1 / 1", both: true)
  ],
)
#set text(font: "Inter", size: 9pt, fill: ink, lang: d.lang)

#let mono(body) = text(font: "JetBrains Mono", size: 7.5pt, body)

#for file in d.files {
  pdf.attach(
    file.name,
    file.data,
    relationship: file.relationship,
    mime-type: file.mime,
    description: file.description,
  )
}

// --- Header.
#grid(
  columns: (1fr, auto),
  align: (left + horizon, right + horizon),
  image(d.logo, height: 0.95cm, format: d.logo_format),
  [
    #text(size: 15pt, weight: "bold", d.title) \
    #text(fill: muted, d.batch_label_label) #mono(d.batch_id) \
    #if d.batch_label != none [#d.batch_label \ ]
    #text(fill: muted, d.period_label) #d.period
  ],
)
#v(4mm)

// --- Totals.
#block(
  width: 100%,
  fill: if d.all_compliant { green } else { red },
  inset: (x: 12pt, y: 9pt),
  radius: 6pt,
  text(fill: white)[
    #text(size: 12.5pt, weight: "bold", d.summary_title) \
    #d.summary
  ],
)
#v(2mm)
#table(
  columns: (32%, 1fr),
  stroke: (x, y) => (bottom: 0.4pt + rule),
  inset: (x: 0pt, y: 5pt),
  ..d.facts.map(fact => (text(fill: muted, fact.label), fact.value)).flatten(),
)

// --- One row per drive.
#v(4mm)
#text(size: 10.5pt, weight: "bold", fill: indigo, d.drives_title)
#v(-2mm)
#line(length: 100%, stroke: 0.6pt + indigo)
#table(
  columns: (auto, auto, 1fr, auto, 1.25fr, auto, auto, auto),
  stroke: (x, y) => (bottom: 0.4pt + rule),
  inset: (x: 3pt, y: 4pt),
  align: (x, y) => if x == 3 { right } else { left },
  table.header(..d.headers.map(h => text(size: 8pt, weight: "bold", fill: muted, h))),
  ..d.rows
    .map(r => (
      text(size: 8pt, r.index),
      mono(r.serial),
      text(size: 8pt, r.model),
      text(size: 8pt, r.capacity),
      text(size: 8pt, r.method),
      text(size: 8pt, r.category),
      text(size: 8pt, weight: "bold", fill: if r.compliant { green } else { red }, r.status),
      text(font: "JetBrains Mono", size: 6.5pt, r.report),
    ))
    .flatten(),
)

#v(4mm)
#grid(
  columns: (1fr, 3cm),
  column-gutter: 6mm,
  [
    #text(size: 8pt, fill: muted, d.verify_hint)
    #v(2mm)
    #text(size: 8pt, fill: muted, d.legal)
  ],
  align(center + bottom, image(d.stamp, width: 3cm, format: "svg")),
)
