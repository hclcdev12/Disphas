// Oblivio erasure certificate (docs/certificat.md §6).
//
// Layout only: every text arrives translated and formatted in `sys.inputs`,
// as data. Nothing typed by an operator is ever evaluated as markup.

#let d = sys.inputs

#let ink = rgb("#111827")
#let muted = rgb("#6B7280")
#let rule = rgb("#D1D5DB")
#let green = rgb("#10B981")
#let red = rgb("#EF4444")
#let indigo = rgb("#6366F1")

#set document(
  title: d.title + " — " + d.report_id,
  date: datetime(
    year: d.year, month: d.month, day: d.day,
    hour: d.hour, minute: d.minute, second: d.second,
  ),
)
#set page(
  paper: "a4",
  margin: (x: 1.8cm, top: 1.6cm, bottom: 1.8cm),
  footer: context text(size: 7.5pt, fill: muted)[
    #d.title · #d.report_id #h(1fr) #counter(page).display("1 / 1", both: true)
  ],
)
#set text(font: "Inter", size: 9.5pt, fill: ink, lang: d.lang)

#let mono(body) = text(font: "JetBrains Mono", size: 8.5pt, body)

// Parenthesized: in markup, a method chain cannot continue on the next line.
#let lines(values, monospace) = (
  values.map(value => if monospace { mono(value) } else { value }).join(linebreak())
)

#let rows-table(rows) = table(
    columns: (32%, 1fr),
    stroke: (x, y) => (bottom: 0.4pt + rule),
    inset: (x: 0pt, y: 5pt),
    ..rows
      .map(row => (
        text(fill: muted, row.label),
        lines(row.value, row.mono),
      ))
      .flatten(),
)

#let section(title, rows) = {
  v(3mm)
  text(size: 10.5pt, weight: "bold", fill: indigo, title)
  v(-2mm)
  line(length: 100%, stroke: 0.6pt + indigo)
  v(-1mm)
  rows-table(rows)
}

#pdf.attach(
  "attestation.cose",
  d.attestation,
  relationship: "source",
  mime-type: "application/cose",
  description: d.attestation_description,
)
#pdf.attach(
  "journal.json",
  d.journal,
  relationship: "supplement",
  mime-type: "application/json",
  description: d.journal_description,
)

// --- Header.
#grid(
  columns: (1fr, auto),
  align: (left + horizon, right + horizon),
  image(d.logo, height: 0.95cm, format: d.logo_format),
  [
    #text(size: 15pt, weight: "bold", d.title) \
    #text(fill: muted, d.report_label) #mono(d.report_id) \
    #text(fill: muted, d.issued_label) #d.issued
  ],
)
#v(4mm)

// --- Status banners.
#for banner in d.banners {
  block(
    width: 100%,
    fill: if banner.kind == "ok" { green } else { red },
    inset: (x: 12pt, y: 9pt),
    radius: 6pt,
    below: 2.5mm,
    text(fill: white)[
      #text(size: 12.5pt, weight: "bold", banner.title)
      #if banner.subtitle != none [ \ #banner.subtitle ]
    ],
  )
}

// --- Forcing justification, shown first when present.
#if d.justification != none {
  block(
    width: 100%,
    stroke: 1pt + red,
    inset: 10pt,
    radius: 6pt,
    [#text(weight: "bold", fill: red, d.justification.label) \ #d.justification.text],
  )
}

#for s in d.sections { section(s.title, s.rows) }

// --- Issuer, fingerprint and QR code.
#v(4mm)
#grid(
  columns: (1fr, 3cm, 5.5cm),
  column-gutter: 6mm,
  [
    #section(d.issuer.title, d.issuer.rows)
    #v(2mm)
    #text(fill: muted, d.fingerprint_label) \
    #mono(d.fingerprint)
    #v(2mm)
    #text(size: 8pt, fill: muted, d.verify_hint)
  ],
  align(center + bottom, image(d.stamp, width: 3cm, format: "svg")),
  align(right + bottom, image(d.qr, width: 5.5cm, format: "svg")),
)

// --- Technical annex.
#pagebreak()
#text(size: 13pt, weight: "bold", d.annex.title)
#v(1mm)
#text(fill: muted, d.annex.intro)
#v(1mm)
#line(length: 100%, stroke: 0.6pt + indigo)
#v(-1mm)
#rows-table(d.annex.rows)
#v(6mm)
#text(size: 8pt, fill: muted, d.legal)
