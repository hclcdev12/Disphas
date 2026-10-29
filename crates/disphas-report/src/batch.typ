// disphas batch certificate (docs/certificat.md §6.1).
//
// A summary of the drives erased in one batch. Each drive keeps its own
// signed attestation, attached to this PDF: the attestations are the proof,
// this document is their rendering. Layout only: every text arrives
// translated in `sys.inputs`, and nothing typed by an operator is evaluated.

#let d = sys.inputs

#let paper = rgb("#FBFAF6")
#let ink = rgb("#111110")
#let muted = rgb("#4E575B")
#let rule = rgb("#C9C7BE")
#let green = rgb("#2F7D4F")
#let red = rgb("#B3261E")

#let margin = 1.5cm
#let band = 1.25cm

#set document(
  title: d.title + " — " + d.batch_id,
  date: datetime(
    year: d.year, month: d.month, day: d.day,
    hour: d.hour, minute: d.minute, second: d.second,
  ),
)

// Same charter as the certificate of one drive: paper and ink, the totals in
// a band across the top of the first page, Klaudia for text, mono for data.
#let sans = ("Klaudia", "Inter")
#let mono(body, size: 7.5pt, fill: ink) = text(font: "JetBrains Mono", size: size, fill: fill, body)
#let tag(body, fill: muted) = text(font: "JetBrains Mono", size: 7pt, tracking: 0.08em, fill: fill, upper(body))

#set page(
  paper: "a4",
  fill: paper,
  margin: (x: margin, top: band + 1.1cm, bottom: 1.8cm),
  background: context if counter(page).get().first() == 1 {
    place(top + left, block(
      width: 100%,
      height: band,
      fill: if d.all_compliant { green } else { red },
      inset: (x: margin),
      align(horizon, grid(
        columns: (auto, 1fr),
        align: (left, right),
        text(font: "JetBrains Mono", size: 11pt, tracking: 0.06em, fill: white, d.summary),
        text(font: sans, size: 12pt, fill: white, d.summary_title),
      )),
    ))
  },
  footer: context mono(size: 7pt, fill: muted)[
    #d.title · #d.batch_id #h(1fr) #counter(page).display("1 / 1", both: true)
  ],
)
#set text(font: sans, size: 9.5pt, fill: ink, lang: d.lang)

#for file in d.files {
  pdf.attach(
    file.name,
    file.data,
    relationship: file.relationship,
    mime-type: file.mime,
    description: file.description,
  )
}

// --- Header: the batch and its period, the logo on the right.
#grid(
  columns: (1fr, auto),
  align: (left + bottom, right + bottom),
  [
    #tag(d.title) \
    #mono(size: 10pt, d.batch_id) \
    #if d.batch_label != none [#d.batch_label \ ]
    #text(size: 9pt, fill: muted)[#d.period_label #d.period]
  ],
  image(d.logo, height: 0.9cm, format: d.logo_format),
)
#v(-1mm)
#line(length: 100%, stroke: 0.8pt + ink)

#table(
  columns: (5.2cm, 1fr),
  stroke: (x, y) => (bottom: 0.4pt + rule),
  inset: (x: 0pt, y: 5pt),
  ..d.facts.map(fact => (mono(size: 8pt, fill: muted, fact.label), mono(size: 8.5pt, fact.value))).flatten(),
)

// --- One row per drive.
#v(5mm)
#tag(d.drives_title)
#v(-1mm)
#table(
  // The report identifier takes two lines: the model and the method get the room.
  columns: (auto, auto, 1fr, auto, 1.5fr, auto, auto, 2.4cm),
  stroke: (x, y) => (bottom: 0.4pt + rule),
  inset: (x: 3pt, y: 4pt),
  align: (x, y) => if x == 3 { right } else { left },
  table.header(..d.headers.map(h => mono(size: 6.5pt, fill: muted, upper(h)))),
  ..d.rows
    .map(r => (
      mono(r.index),
      if r.hardware == none { mono(r.serial) } else [
        #mono(r.serial) \
        #mono(size: 6.5pt, fill: muted, r.hardware)
      ],
      mono(r.model),
      mono(r.capacity),
      mono(r.method),
      mono(r.category),
      mono(fill: if r.compliant { green } else { red }, r.status),
      // A UUID on two lines, cut after its third group.
      {
        let parts = r.report.split("-")
        if parts.len() > 3 {
          mono(size: 6.5pt)[#parts.slice(0, 3).join("-")- \ #parts.slice(3).join("-")]
        } else { mono(size: 6.5pt, r.report) }
      },
    ))
    .flatten(),
)

#v(5mm)
#text(size: 8.5pt, fill: muted, d.verify_hint)
#v(2mm)
#text(size: 8pt, fill: muted, d.legal)
