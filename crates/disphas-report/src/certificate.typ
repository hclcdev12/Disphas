// disphas erasure certificate (docs/certificat.md §6), in the brand charter
// (dossier de marque, IDENTITE.md §5.3 and §6.4): always paper and ink, the
// status in a band across the top, Klaudia for text, JetBrains Mono for data.
//
// Layout only: every text arrives translated and formatted in `sys.inputs`,
// as data. Nothing typed by an operator is ever evaluated as markup.

#let d = sys.inputs

#let paper = rgb("#FBFAF6")
#let ink = rgb("#111110")
#let muted = rgb("#4E575B")
#let rule = rgb("#C9C7BE")
#let green = rgb("#2F7D4F")
#let red = rgb("#B3261E")

#let margin = 1.8cm
#let band = 1.25cm
#let status = d.banners.first()
#let status-fill = if status.kind == "ok" { green } else { red }

#set document(
  title: d.title + " — " + d.report_id,
  date: datetime(
    year: d.year, month: d.month, day: d.day,
    hour: d.hour, minute: d.minute, second: d.second,
  ),
)

// Klaudia for text; Inter only stands in for a character Klaudia lacks.
#let sans = ("Klaudia", "Inter")
#let mono(body, size: 8.5pt, fill: ink) = text(font: "JetBrains Mono", size: size, fill: fill, body)
// Small technical labels: mono capitals, spaced by 8 % (charter §4.2).
#let tag(body, fill: muted) = text(font: "JetBrains Mono", size: 7pt, tracking: 0.08em, fill: fill, upper(body))

// The status band runs across the top of the first page, edge to edge.
#set page(
  paper: "a4",
  fill: paper,
  margin: (x: margin, top: band + 1.1cm, bottom: 1.8cm),
  background: context if counter(page).get().first() == 1 {
    place(top + left, block(
      width: 100%,
      height: band,
      fill: status-fill,
      inset: (x: margin),
      align(horizon, grid(
        columns: (auto, 1fr),
        align: (left, right),
        text(font: "JetBrains Mono", size: 13pt, tracking: 0.08em, fill: white, status.title),
        if status.subtitle != none { text(font: sans, size: 12pt, fill: white, status.subtitle) },
      )),
    ))
  },
  footer: context mono(size: 7pt, fill: muted)[
    #d.title · #d.report_id #h(1fr) #counter(page).display("1 / 1", both: true)
  ],
)
#set text(font: sans, size: 10pt, fill: ink, lang: d.lang)

// Every value is data, in mono as in the charter's mockup: Klaudia's figures
// (1 and I, 0 and O) are too close for a serial number, a model or a size.
// Parenthesized: in markup, a method chain cannot continue on the next line.
#let lines(values) = (
  values.map(value => mono(value)).join(linebreak())
)

// Same label column everywhere, the issuer's beside the QR code included.
#let rows-table(rows) = table(
  columns: (5.2cm, 1fr),
  stroke: (x, y) => (bottom: 0.4pt + rule),
  inset: (x: 0pt, y: 5pt),
  ..rows
    .map(row => (
      mono(size: 8pt, fill: muted, row.label),
      lines(row.value),
    ))
    .flatten(),
)

#let section(title, rows) = {
  v(4mm)
  tag(title)
  v(-1mm)
  rows-table(rows)
}

// A warning under the band: outlined in red, never tinted.
#let outlined(body) = block(
  width: 100%,
  stroke: 0.8pt + red,
  inset: (x: 10pt, y: 8pt),
  radius: 2pt,
  below: 2mm,
  body,
)

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

// --- Header: what this is and its identifier, the logo on the right.
#grid(
  columns: (1fr, auto),
  align: (left + bottom, right + bottom),
  [
    #tag(d.title) \
    #mono(size: 10pt, d.report_id) \
    #text(size: 9pt, fill: muted)[#d.issued_label #d.issued]
  ],
  image(d.logo, height: 0.9cm, format: d.logo_format),
)
#v(-1mm)
#line(length: 100%, stroke: 0.8pt + ink)

// --- Further banners (destruction recommended), then the forcing justification.
#for banner in d.banners.slice(1) {
  outlined(text(weight: "bold", fill: red, banner.title))
}
#if d.justification != none {
  outlined[#text(weight: "bold", fill: red, d.justification.label) \ #d.justification.text]
}

#for s in d.sections { section(s.title, s.rows) }

// --- Issuer and proof: the QR code holds the signed attestation.
#v(4mm)
#grid(
  columns: (1fr, 5.2cm),
  column-gutter: 8mm,
  [
    #section(d.issuer.title, d.issuer.rows)
    #v(3mm)
    #mono(size: 8pt)[signature Ed25519] \
    #mono(size: 8pt, fill: muted, d.fingerprint_label) #mono(size: 8pt, d.fingerprint)
    #v(2mm)
    #text(size: 8.5pt, fill: muted, d.verify_hint)
  ],
  align(right + bottom, image(d.qr, width: 5.2cm, format: "svg")),
)

// --- Technical annex.
#pagebreak()
#tag(d.annex.title, fill: ink)
#v(1mm)
#text(fill: muted, d.annex.intro)
#v(1mm)
#line(length: 100%, stroke: 0.8pt + ink)
#v(-1mm)
#rows-table(d.annex.rows)
#v(6mm)
#text(size: 8pt, fill: muted, d.legal)
